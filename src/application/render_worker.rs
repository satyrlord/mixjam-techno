//! The worker thread that prepares the mix of a project revision off the calling thread.
use super::Project;
use crate::{
    audio::{self},
    library::Library,
};
use std::sync::{Arc, mpsc};

pub(super) struct RenderRequest {
    pub(super) revision: u64,
    pub(super) quality: crate::playback_quality::PlaybackQuality,
    pub(super) project: Project,
    pub(super) library: Arc<Library>,
}

pub(super) struct RenderResult {
    pub(super) revision: u64,
    pub(super) mix: Result<Arc<audio::Mix>, String>,
}

pub(super) struct RenderWorker {
    pub(super) sender: mpsc::Sender<RenderRequest>,
    pub(super) receiver: mpsc::Receiver<RenderResult>,
    pub(super) in_flight: bool,
    pub(super) current_revision: Option<u64>,
    pub(super) pending: Option<RenderRequest>,
}

impl RenderWorker {
    pub(super) fn new() -> Self {
        let (sender, receiver) = Self::spawn();
        Self {
            sender,
            receiver,
            in_flight: false,
            current_revision: None,
            pending: None,
        }
    }

    fn spawn() -> (mpsc::Sender<RenderRequest>, mpsc::Receiver<RenderResult>) {
        let (sender, requests) = mpsc::channel::<RenderRequest>();
        let (results, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("mixjam-techno-render".into())
            .spawn(move || {
                // An edit rarely changes which sounds a mix uses. Keep their
                // decoded audio for the next render of the same library.
                let mut decoded = std::collections::HashMap::new();
                let mut decoded_from = std::sync::Weak::new();
                while let Ok(request) = requests.recv() {
                    let library = Arc::downgrade(&request.library);
                    if !decoded_from.ptr_eq(&library) {
                        decoded.clear();
                        decoded_from = library;
                    }
                    let mix = audio::prepare_project_reusing_with_quality(
                        &request.project,
                        &request.library,
                        &mut decoded,
                        request.quality,
                    )
                    .map(Arc::new);
                    if results
                        .send(RenderResult {
                            revision: request.revision,
                            mix,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("Cannot create the audio render worker.");
        (sender, receiver)
    }

    pub(super) fn request(&mut self, request: RenderRequest) -> Result<(), String> {
        if self.current_revision == Some(request.revision) {
            return Ok(());
        }
        if self.in_flight {
            self.pending = Some(request);
        } else {
            self.current_revision = Some(request.revision);
            self.sender
                .send(request)
                .map_err(|_| "The audio render worker stopped.".to_string())?;
            self.in_flight = true;
        }
        Ok(())
    }

    pub(super) fn poll(&mut self) -> Option<RenderResult> {
        let result = match self.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                // The worker died during a render. Report that render as failed
                // and replace the worker, so later requests are not stranded.
                (self.sender, self.receiver) = Self::spawn();
                RenderResult {
                    revision: self.current_revision?,
                    mix: Err("The audio render worker stopped.".into()),
                }
            }
        };
        self.in_flight = false;
        self.current_revision = None;
        if let Some(request) = self.pending.take() {
            let revision = request.revision;
            if self.sender.send(request).is_ok() {
                self.in_flight = true;
                self.current_revision = Some(revision);
            }
        }
        Some(result)
    }

    pub(super) fn cancel_pending(&mut self) {
        self.pending = None;
    }
}
