use super::*;
use super::{client::words, server::key_path};
use crate::album_review;
use std::{
    io::{BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc,
    time::Duration,
};

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

fn sample_review(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "kind": "reference",
        "artist": "Artist",
        "title": "Album",
        "year": 1990,
        "label": "Label",
        "producer": "Producer",
        "genre": "Techno",
        "genreTags": ["Techno"],
        "runtime": "6:00",
        "overview": "An overview.",
        "tracks": [{
            "num": 1,
            "title": "Drive",
            "duration": "6:00",
            "energy": "peak",
            "tags": ["Acid", "Four-on-the-floor"],
            "role": "Album role: The opener.",
            "events": [
                {"timestamp": "0:00", "section": "Intro", "description": "Kick alone."},
                {"timestamp": "1:30", "section": "Drop", "description": "The acid line."}
            ]
        }]
    })
}

#[test]
fn album_review_requests_list_read_query_add_and_update_reviews() {
    let folder = key_folder("reviews");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut shelf = album_review::Shelf::open(Some(folder.clone()));
    let request = |words: &[&str]| {
        serde_json::from_str::<Request>(&request_line(&strings(words)).unwrap()).unwrap()
    };
    let mut review = sample_review("artist-album");
    let line = format!(r#"{{"command":"add_review","review":{review}}}"#);
    let add = serde_json::from_str::<Request>(&request_line(&[line]).unwrap()).unwrap();
    match album_reply(&mut shelf, &add).unwrap().unwrap() {
        Body::ReviewSaved { id, path, added } => {
            assert_eq!(id, "artist-album");
            assert!(added);
            assert_eq!(path, folder.join("artist-album.json"));
            assert!(path.is_file());
        }
        other => panic!("Unexpected reply {other:?}"),
    }
    // Adding the same id again fails, and so does a review that breaks a rule.
    assert!(album_reply(&mut shelf, &add).unwrap().is_err());
    review["id"] = "other-album".into();
    review["tracks"][0]["role"] = "No prefix".into();
    let broken = Request::AddReview {
        review: Some(review.clone()),
        file: None,
    };
    let error = album_reply(&mut shelf, &broken).unwrap().unwrap_err();
    assert!(error.contains("Album role"), "{error}");
    assert!(!folder.join("other-album.json").exists());
    // Update from a file.
    let file = folder.join("draft.txt");
    let mut changed = sample_review("artist-album");
    changed["title"] = "Album (Remastered)".into();
    std::fs::write(&file, changed.to_string()).unwrap();
    let update = request(&["update_review", &format!("file={}", file.display())]);
    assert!(matches!(
        album_reply(&mut shelf, &update).unwrap().unwrap(),
        Body::ReviewSaved { added: false, .. }
    ));
    match album_reply(&mut shelf, &request(&["review", "id=artist-album"])) {
        Some(Ok(Body::Review { review })) => assert_eq!(review.title, "Album (Remastered)"),
        other => panic!("Unexpected reply {other:?}"),
    }
    assert!(
        album_reply(&mut shelf, &request(&["review", "id=nope"]))
            .unwrap()
            .is_err()
    );
    match album_reply(
        &mut shelf,
        &request(&["reviews", "tag=acid", "energy=PEAK"]),
    ) {
        Some(Ok(Body::Reviews {
            total,
            reviews,
            folder: listed,
            problems,
        })) => {
            assert_eq!(total, 1);
            assert_eq!(reviews[0].id, "artist-album");
            assert_eq!(reviews[0].tracks, 1);
            assert_eq!(listed, Some(folder.clone()));
            assert!(problems.is_empty());
        }
        other => panic!("Unexpected reply {other:?}"),
    }
    match album_reply(
        &mut shelf,
        &request(&["review_tracks", "energy=peak", "artist=art"]),
    ) {
        Some(Ok(Body::ReviewTracks { total, tracks })) => {
            assert_eq!(total, 1);
            assert_eq!(tracks[0].track.events[1].section, "Drop");
        }
        other => panic!("Unexpected reply {other:?}"),
    }
    let none = request(&["review_tracks", "energy=low"]);
    assert!(matches!(
        album_reply(&mut shelf, &none),
        Some(Ok(Body::ReviewTracks { total: 0, .. }))
    ));
    for bad in [
        &["reviews", "energy=loud"][..],
        &["reviews", "kind=theirs"],
        &["reviews", "sort=random"],
    ] {
        assert!(
            album_reply(&mut shelf, &request(bad)).unwrap().is_err(),
            "{bad:?}"
        );
    }
    let both = Request::AddReview {
        review: Some(sample_review("x-y")),
        file: Some(file.clone()),
    };
    assert!(album_reply(&mut shelf, &both).unwrap().is_err());
    assert!(album_reply(&mut shelf, &Request::State).is_none());
    // The reply serializes as the wire expects.
    let reply = encode(&album_reply(&mut shelf, &request(&["reviews"])).unwrap());
    assert!(reply.contains(r#""genre_tags":["Techno"]"#), "{reply}");
    assert!(reply.contains(r#""kind":"reference""#), "{reply}");
    assert!(!reply.contains("problems"), "{reply}");
    std::fs::remove_dir_all(folder).unwrap();
}

fn key_folder(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mixjam-agent-{}-{tag}", std::process::id()))
}

#[test]
fn capture_takes_an_optional_reason_and_replies_with_its_folder() {
    let parse =
        |words: &[&str]| serde_json::from_str::<Request>(&request_line(&strings(words)).unwrap());
    assert_eq!(
        parse(&["capture"]).unwrap(),
        Request::Capture { reason: None }
    );
    assert_eq!(
        parse(&["capture", "reason=Noise after bar 65"]).unwrap(),
        Request::Capture {
            reason: Some("Noise after bar 65".into())
        }
    );
    assert!(request_line(&strings(&["capture", "seconds=10"])).is_err());
    assert!(HELP.contains("capture [reason=]"));
    let reply = serde_json::to_value(Body::Capture {
        folder: PathBuf::from("diagnostics/2026-11-07_21-30-00"),
        state: serde_json::json!({"reason": "Saved by an agent"}),
    })
    .unwrap();
    assert_eq!(reply["folder"], "diagnostics/2026-11-07_21-30-00");
    assert_eq!(reply["state"]["reason"], "Saved by an agent");
}

#[test]
fn command_words_become_typed_requests_and_reject_unknown_fields() {
    let line = request_line(&strings(&[
        "place",
        "sample=Analog kick",
        "track=2",
        "bar=1.5",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Place {
            sample: Value::Text("Analog kick".into()),
            track: 2,
            bar: Some(1.5),
            tick: None,
        }
    );
    // A label that looks like a number stays a label.
    let line = request_line(&strings(&["click", "target=160", "double=true"])).unwrap();
    assert!(matches!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Click { target, double: true, pointer: false, .. } if target == "160"
    ));
    let line = request_line(&strings(&["move_clips", "clips=[3,4]", "bars=-2"])).unwrap();
    assert!(matches!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::MoveClips { clips, bars, copy: false, .. } if clips == [3, 4] && bars == -2.0
    ));
    let line = request_line(&strings(&[
        r#"{"command":"set_loop","first_bar":1,"last_bar":8}"#,
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::SetLoop {
            first_bar: Some(1),
            last_bar: Some(8)
        }
    );
    assert_eq!(
        serde_json::from_str::<Request>(&request_line(&strings(&["state"])).unwrap()).unwrap(),
        Request::State
    );
    let line = request_line(&strings(&[
        "automate",
        "track=3",
        "lane=send2",
        "points=[[1,0],[4.5,0.75]]",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Automate {
            track: Some(3),
            bus: None,
            lane: "send2".into(),
            points: vec![[1.0, 0.0], [4.5, 0.75]],
        }
    );
    let line = request_line(&strings(&[
        "automate",
        "track=5",
        "lane=pan",
        "points=[[1,-1],[1.25,1],[2,0]]",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Automate {
            track: Some(5),
            bus: None,
            lane: "pan".into(),
            points: vec![[1.0, -1.0], [1.25, 1.0], [2.0, 0.0]],
        }
    );
    let line = request_line(&strings(&[
        "automate",
        "track=2",
        "lane=repeat",
        "points=[[5,8],[5.25,16],[5.5,32],[6,0]]",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Automate {
            track: Some(2),
            bus: None,
            lane: "repeat".into(),
            points: vec![[5.0, 8.0], [5.25, 16.0], [5.5, 32.0], [6.0, 0.0]],
        }
    );
    let line = request_line(&strings(&[
        "automate",
        "track=2",
        "lane=repeatgate",
        "points=[[5,25],[5.5,100]]",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Automate {
            track: Some(2),
            bus: None,
            lane: "repeatgate".into(),
            points: vec![[5.0, 25.0], [5.5, 100.0]],
        }
    );
    let line = request_line(&strings(&[
        "automate",
        "track=2",
        "lane=pitch",
        "points=[[7.5,0],[8,-4800]]",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Automate {
            track: Some(2),
            bus: None,
            lane: "pitch".into(),
            points: vec![[7.5, 0.0], [8.0, -4800.0]],
        }
    );
    let line = request_line(&strings(&[
        "automate",
        "bus=3",
        "lane=return",
        "points=[[8,0.2],[8.75,2],[9,0.2]]",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Automate {
            track: None,
            bus: Some(3),
            lane: "return".into(),
            points: vec![[8.0, 0.2], [8.75, 2.0], [9.0, 0.2]],
        }
    );
    let line = request_line(&strings(&[
        "clip",
        "clip=7",
        "gain_db=-3.5",
        "fade_out_ticks=128",
        "fade_out_curve=equal-power",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Clip {
            clip: 7,
            gain_db: Some(-3.5),
            offset_frames: None,
            length_ticks: None,
            fade_in_ticks: None,
            fade_in_curve: None,
            fade_out_ticks: Some(128),
            fade_out_curve: Some(crate::engine::FadeCurve::EqualPower),
            pitch_cents: None,
            reverse: None,
        }
    );
    let line = request_line(&strings(&["clip", "clip=7", "reverse=true"])).unwrap();
    assert!(matches!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::Clip {
            clip: 7,
            reverse: Some(true),
            gain_db: None,
            ..
        }
    ));
    let line = request_line(&strings(&[
        "set_libraries",
        r#"folders=["C:/one","D:/two"]"#,
        "fallback_bpm=160",
    ]))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::SetLibraries {
            folders: vec![PathBuf::from("C:/one"), PathBuf::from("D:/two")],
            fallback_bpm: Some(160.0),
        }
    );
    // One Windows path is text, not JSON.
    let line = request_line(&strings(&["set_libraries", r"folders=C:\one folder"])).unwrap();
    assert_eq!(
        serde_json::from_str::<Request>(&line).unwrap(),
        Request::SetLibraries {
            folders: vec![PathBuf::from(r"C:\one folder")],
            fallback_bpm: None,
        }
    );
    for invalid in [
        &["place", "sample=1", "track=1", "bars=1"][..],
        &["frobnicate"],
        &["click"],
        &["state", "loose"],
        &["clip", "clip=1", "fade_in_curve=sharp"],
        &[],
    ] {
        assert!(request_line(&strings(invalid)).is_err(), "{invalid:?}");
    }
    assert_eq!(
        words(r#"set target="Find a sound" value="deep  bass" window=Sounds"#),
        [
            "set",
            "target=Find a sound",
            "value=deep  bass",
            "window=Sounds"
        ]
    );
    assert_eq!(words(r#"type text="""#), ["type", "text="]);
}

#[test]
fn positions_count_bars_from_one_and_ticks_from_zero() {
    assert_eq!(position_tick(Some(1.0), None), Ok(0));
    assert_eq!(position_tick(Some(3.25), None), Ok(2 * 256 + 64));
    assert_eq!(position_tick(None, Some(96)), Ok(96));
    assert!(position_tick(Some(0.5), None).is_err());
    assert!(position_tick(Some(f64::NAN), None).is_err());
    assert!(position_tick(Some(1.0), Some(0)).is_err());
    assert!(position_tick(None, None).is_err());
}

#[test]
fn replies_are_one_flat_object_with_the_result() {
    let reply: serde_json::Value = serde_json::from_str(&encode(&Ok(Body::Samples {
        total: 0,
        samples: Vec::new(),
    })))
    .unwrap();
    assert_eq!(
        reply,
        serde_json::json!({"ok": true, "total": 0, "samples": []})
    );
    let reply: serde_json::Value =
        serde_json::from_str(&encode(&Err("No such control.".into()))).unwrap();
    assert_eq!(
        reply,
        serde_json::json!({"ok": false, "error": "No such control."})
    );
}

#[test]
fn dropping_server_releases_the_listener_and_idle_connections() {
    let folder = key_folder("shutdown");
    let server = Server::start(0, &folder).unwrap();
    let port = server.port();
    let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let accepted = loop {
        if let Some(connection) = server
            .connections
            .lock()
            .unwrap()
            .iter()
            .find_map(std::sync::Weak::upgrade)
        {
            break connection;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "The listener did not accept the idle test client."
        );
        std::thread::sleep(Duration::from_millis(1));
    };
    // This client never sends its key. Shutdown must still close it.
    // Keep the accepted socket alive to require explicit shutdown.
    drop(server);
    assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
    let mut byte = [0];
    assert!(matches!(client.read(&mut byte), Ok(0)));
    drop(accepted);
    assert!(!key_path(&folder, port).exists());
    std::fs::remove_dir(folder).unwrap();
}

#[test]
fn server_queues_requests_wakes_the_frontend_and_returns_its_replies() {
    let folder = key_folder("queue");
    let server = Server::start_with(0, &folder, Duration::from_millis(150)).unwrap();
    let key = server.key().to_string();
    assert_eq!(
        std::fs::read_to_string(key_path(&folder, server.port())).unwrap(),
        key
    );
    let (woken, wakes) = mpsc::channel();
    server.set_waker(move || {
        let _ = woken.send(());
    });
    let port = server.port();
    let client_key = key.clone();
    let client = std::thread::spawn(move || {
        let mut output = Vec::new();
        let ok = send(
            port,
            &client_key,
            [
                request_line(&strings(&["help"])),
                request_line(&strings(&["seek", "bar=5"])),
                // The client stops before this one.
                request_line(&strings(&["state"])),
            ],
            &mut output,
        )
        .unwrap();
        (ok, String::from_utf8(output).unwrap())
    });
    for expected in [
        Request::Help,
        Request::Seek {
            bar: Some(5.0),
            tick: None,
        },
    ] {
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
        let (request, respond) = server.next().unwrap();
        assert_eq!(request, expected);
        respond.send(match request {
            Request::Help => Ok(Body::Help { help: HELP }),
            _ => Err("The song is empty.".into()),
        });
    }
    let (ok, output) = client.join().unwrap();
    let replies: Vec<serde_json::Value> = output
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    // The client stops at the failed request.
    assert!(!ok);
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["ok"], true);
    assert_eq!(replies[1]["error"], "The song is empty.");

    // A request that the frontend does not take in time fails, and the
    // frontend can no longer run it.
    let mut output = Vec::new();
    assert!(
        !send(
            port,
            &key,
            [request_line(&strings(&["state"]))],
            &mut output
        )
        .unwrap()
    );
    assert!(String::from_utf8(output).unwrap().contains("did not take"));
    assert!(server.next().is_none());

    // The key file lasts as long as the session.
    drop(server);
    assert!(!key_path(&folder, port).exists());
    std::fs::remove_dir(folder).unwrap();
}

#[test]
fn a_connection_without_the_key_or_with_a_malformed_line_runs_nothing_more() {
    let folder = key_folder("refuse");
    let server = Server::start_with(0, &folder, Duration::from_secs(5)).unwrap();
    let exchange = |bytes: &[u8]| {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port())).unwrap();
        stream.write_all(bytes).unwrap();
        let mut reply = String::new();
        BufReader::new(stream).read_to_string(&mut reply).unwrap();
        reply
    };
    for intruder in [
        &b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\n{\"command\":\"quit\"}\n"[..],
        b"{\"command\":\"quit\"}\n",
        b"0000000000000000\n{\"command\":\"quit\"}\n",
    ] {
        let reply = exchange(intruder);
        assert_eq!(reply.lines().count(), 1);
        assert!(reply.contains("session key is wrong"), "{reply}");
    }
    let reply = exchange(
        format!(
            "{}\nnot a request\n{{\"command\":\"quit\"}}\n",
            server.key()
        )
        .as_bytes(),
    );
    assert_eq!(reply.lines().count(), 1);
    assert!(reply.contains("Invalid request"), "{reply}");
    assert!(server.next().is_none());
    drop(server);
    std::fs::remove_dir(folder).unwrap();
}
