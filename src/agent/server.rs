//! The loopback server that accepts requests and queues them for a frontend.
use super::{Body, Request, encode};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

/// How long a request may wait for the window to take it.
const START_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REQUEST_BYTES: u64 = 1 << 20;

/// Returns the reply of one request to the client that sent it.
pub struct Responder(mpsc::Sender<String>);

impl Responder {
    pub fn send(self, reply: Result<Body, String>) {
        // The client may have disconnected. Its reply then has no reader.
        let _ = self.0.send(encode(&reply));
    }
}

struct Incoming {
    request: Request,
    /// Set by whichever side acts first: the window when it takes the
    /// request, or the connection when it stops waiting.
    claimed: Arc<AtomicBool>,
    reply: mpsc::Sender<String>,
}

type Waker = Arc<Mutex<Option<Box<dyn Fn() + Send>>>>;

/// Where a session on `port` keeps its key, in the application's data folder.
pub(super) fn key_path(folder: &Path, port: u16) -> PathBuf {
    folder.join(format!("agent-{port}.key"))
}

/// A key that another program cannot guess. The standard library seeds each
/// hasher from the operating system's random source.
fn new_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..4)
        .map(|_| {
            let state = std::collections::hash_map::RandomState::new();
            format!("{:016x}", state.build_hasher().finish())
        })
        .collect()
}

/// Accepts requests on the loopback interface and queues them for a frontend.
pub struct Server {
    port: u16,
    key: String,
    key_file: PathBuf,
    incoming: mpsc::Receiver<Incoming>,
    waker: Waker,
    stopping: Arc<AtomicBool>,
    listener: Option<std::thread::JoinHandle<()>>,
    pub(super) connections: Arc<Mutex<Vec<std::sync::Weak<TcpStream>>>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        // Remove the key before releasing the port, so a replacement session's
        // key cannot be removed by this session's cleanup.
        let _ = std::fs::remove_file(&self.key_file);
        self.stopping.store(true, Ordering::SeqCst);
        // Wake accept so the listener releases the port before Drop returns.
        // Without that connection accept may never return, and waiting for
        // it would keep the window from closing.
        if TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).is_ok()
            && let Some(listener) = self.listener.take()
        {
            let _ = listener.join();
        }
        if let Ok(connections) = self.connections.lock() {
            for connection in connections.iter().filter_map(std::sync::Weak::upgrade) {
                let _ = connection.shutdown(std::net::Shutdown::Both);
            }
        }
        if let Ok(mut wake) = self.waker.lock() {
            *wake = None;
        }
    }
}

impl Server {
    /// Listen on `port` and write the session key to `folder`. A client must
    /// send the key first. The file uses default file system permissions.
    pub fn start(port: u16, folder: &Path) -> Result<Self, String> {
        Self::start_with(port, folder, START_TIMEOUT)
    }

    pub(super) fn start_with(
        port: u16,
        folder: &Path,
        start_timeout: Duration,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .map_err(|error| format!("Cannot listen for agents on port {port}: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        let key = new_key();
        let key_file = key_path(folder, port);
        std::fs::create_dir_all(folder)
            .map_err(|error| error.to_string())
            .and_then(|()| crate::atomic_file::write(&key_file, key.as_bytes()))
            .map_err(|error| format!("Cannot write the agent key: {error}"))?;
        let (sender, incoming) = mpsc::channel();
        let waker = Waker::default();
        let wake = waker.clone();
        let expected = key.clone();
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let connections = Arc::new(Mutex::new(Vec::<std::sync::Weak<TcpStream>>::new()));
        let clients = connections.clone();
        let listener = std::thread::Builder::new()
            .name("mixjam-techno-agent".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let stream = Arc::new(stream);
                    if let Ok(mut clients) = clients.lock() {
                        clients.retain(|client| client.strong_count() > 0);
                        clients.push(Arc::downgrade(&stream));
                    }
                    let sender = sender.clone();
                    let wake = wake.clone();
                    let expected = expected.clone();
                    let _ = std::thread::Builder::new()
                        .name("mixjam-techno-agent-client".into())
                        .spawn(move || serve(stream, &expected, &sender, &wake, start_timeout));
                }
            })
            .map_err(|error| {
                let _ = std::fs::remove_file(&key_file);
                format!("Cannot start the agent listener: {error}")
            })?;
        Ok(Self {
            port,
            key,
            key_file,
            incoming,
            waker,
            stopping,
            listener: Some(listener),
            connections,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    /// Call `waker` from the connection thread whenever a request arrives.
    pub fn set_waker(&self, waker: impl Fn() + Send + 'static) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(Box::new(waker));
        }
    }

    /// The next request that a client still waits for.
    pub fn next(&self) -> Option<(Request, Responder)> {
        loop {
            let incoming = self.incoming.try_recv().ok()?;
            if !incoming.claimed.swap(true, Ordering::SeqCst) {
                return Some((incoming.request, Responder(incoming.reply)));
            }
        }
    }
}

fn serve(
    stream: Arc<TcpStream>,
    key: &str,
    requests: &mpsc::Sender<Incoming>,
    waker: &Waker,
    start_timeout: Duration,
) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream.as_ref());
    // The first line is the session key. Anything else ends the connection,
    // which also stops a request that another program was tricked into
    // sending, such as an HTTP form post.
    let mut first = String::new();
    if (&mut reader)
        .take(MAX_REQUEST_BYTES)
        .read_line(&mut first)
        .is_err()
        || first.trim_end() != key
    {
        let _ = writeln!(
            writer,
            "{}",
            encode(&Err(
                "The session key is wrong. Send requests with the `agent` command.".into()
            ))
        );
        return;
    }
    loop {
        let mut line = String::new();
        match (&mut reader).take(MAX_REQUEST_BYTES).read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }
        // A malformed line ends the connection.
        let request = if line.ends_with('\n') || (line.len() as u64) < MAX_REQUEST_BYTES {
            serde_json::from_str::<Request>(&line)
                .map_err(|error| format!("Invalid request: {error}"))
        } else {
            Err("The request is too long.".into())
        };
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                let _ = writeln!(writer, "{}", encode(&Err(error)));
                return;
            }
        };
        let (reply, replies) = mpsc::channel();
        let claimed = Arc::new(AtomicBool::new(false));
        let queued = requests
            .send(Incoming {
                request,
                claimed: claimed.clone(),
                reply,
            })
            .is_ok();
        if let Ok(waker) = waker.lock()
            && let Some(wake) = waker.as_ref()
        {
            wake();
        }
        let closed = || encode(&Err("The application closed before it replied.".into()));
        let reply = if queued {
            loop {
                match replies.recv_timeout(start_timeout) {
                    Ok(reply) => break reply,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break closed(),
                    // The window has the request and reports when it is done.
                    Err(mpsc::RecvTimeoutError::Timeout)
                        if claimed.swap(true, Ordering::SeqCst) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        break encode(&Err(
                            "The window did not take the request. Restore it if it is minimized."
                                .into(),
                        ));
                    }
                }
            }
        } else {
            closed()
        };
        if writeln!(writer, "{reply}").is_err() {
            return;
        }
    }
}
