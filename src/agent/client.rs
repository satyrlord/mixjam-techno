//! The command-line client: it turns command words into request lines and exchanges them with a server.
use super::{DEFAULT_PORT, HELP, Request, server::key_path};
use std::{
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, TcpStream},
};

/// Split a request line into words. Double quotes keep spaces in a word.
pub(super) fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    for character in line.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            character if character.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            character => {
                word.push(character);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// The JSON line for `COMMAND name=value ...`, or for one JSON object.
pub fn request_line(words: &[String]) -> Result<String, String> {
    let (first, rest) = words
        .split_first()
        .ok_or("Give a command. Use `agent help` for the list.")?;
    let value = if first.trim_start().starts_with('{') {
        if !rest.is_empty() {
            return Err("Give one JSON object, or a command with name=value fields.".into());
        }
        serde_json::from_str(first).map_err(|error| format!("Invalid JSON: {error}"))?
    } else {
        let mut fields = serde_json::Map::new();
        fields.insert("command".into(), first.as_str().into());
        for word in rest {
            let (name, value) = word
                .split_once('=')
                .ok_or_else(|| format!("Write `{word}` as name=value."))?;
            fields.insert(
                name.into(),
                serde_json::from_str(value).unwrap_or_else(|_| value.into()),
            );
        }
        serde_json::Value::Object(fields)
    };
    serde_json::from_value::<Request>(value.clone())
        .map_err(|error| format!("Invalid request: {error}. Use `agent help` for the list."))?;
    Ok(value.to_string())
}

/// Send request lines and write each reply. Stops at the first failed
/// request and returns whether every request succeeded.
pub fn send(
    port: u16,
    key: &str,
    lines: impl IntoIterator<Item = Result<String, String>>,
    output: &mut impl Write,
) -> Result<bool, String> {
    let mut connection: Option<(TcpStream, BufReader<TcpStream>)> = None;
    for line in lines {
        let line = line?;
        if connection.is_none() {
            let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).map_err(|error| {
                format!(
                    "Cannot reach MixJam Techno on port {port}: {error}. Start it with `gui \
                     --agent`."
                )
            })?;
            let reader = BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
            writeln!(&stream, "{key}")
                .map_err(|error| format!("Cannot send the session key: {error}"))?;
            connection = Some((stream, reader));
        }
        let (stream, reader) = connection
            .as_mut()
            .expect("The connection was just opened.");
        writeln!(stream, "{line}").map_err(|error| format!("Cannot send the request: {error}"))?;
        let mut reply = String::new();
        reader
            .read_line(&mut reply)
            .map_err(|error| format!("Cannot read the reply: {error}"))?;
        if reply.is_empty() {
            return Err("The application closed the connection.".into());
        }
        output
            .write_all(reply.as_bytes())
            .and_then(|()| output.flush())
            .map_err(|error| error.to_string())?;
        let ok = serde_json::from_str::<serde_json::Value>(&reply)
            .ok()
            .and_then(|reply| reply.get("ok")?.as_bool())
            .unwrap_or(false);
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The `agent` command: `[--port N] COMMAND [name=value ...]`, or `-` to read
/// request lines from standard input.
pub fn run_client(arguments: &[String]) -> Result<bool, String> {
    let mut port = DEFAULT_PORT;
    let mut rest = arguments;
    if let [flag, value, tail @ ..] = rest
        && flag == "--port"
    {
        port = value
            .parse()
            .map_err(|_| "--port needs a port number.".to_string())?;
        rest = tail;
    }
    let mut output = std::io::stdout().lock();
    if matches!(rest, [word] if word == "help") {
        writeln!(output, "{HELP}").map_err(|error| error.to_string())?;
        return Ok(true);
    }
    let key_file = key_path(&crate::project_io::data_root(), port);
    let key = std::fs::read_to_string(&key_file).map_err(|_| {
        format!(
            "No session key at {}. Start the application with `gui --agent`, with the same port \
             and the same MIXJAM_TECHNO_DATA_DIR as this command.",
            key_file.display()
        )
    })?;
    let key = key.trim();
    if matches!(rest, [word] if word == "-") {
        let lines = std::io::stdin().lines().filter_map(|line| match line {
            Ok(line) if line.trim().is_empty() || line.trim_start().starts_with('#') => None,
            Ok(line) if line.trim_start().starts_with('{') => Some(request_line(&[line])),
            Ok(line) => Some(request_line(&words(&line))),
            Err(error) => Some(Err(error.to_string())),
        });
        return send(port, key, lines, &mut output);
    }
    send(port, key, [request_line(rest)], &mut output)
}
