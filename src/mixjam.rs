//! `.mixjam` songs: the application's own format version 9 (`song`) and a
//! bounded import of the preserved MixJam JSON v7 format.
//!
//! The v7 reader imports an arrangement into the shared renderer; it is not MixJam DSP
//! emulation. Master processors, return effects and edge fades remain serialized
//! in source notices and are not rendered; the importer states this sound change.
//! Muting/solo becomes channel routing, and later same-lane triggers truncate
//! earlier clips. Playback tempo comes from the measured WAV duration and saved
//! placement span, as in MixJam's stretchRatioForDuration scheduling. Descriptive
//! nativeBPM is not the arrangement playback-rate authority. UI names, palette,
//! generator metadata and bypassed processor settings are not native controls.
mod references;
mod song;
pub(crate) use references::ReferenceResolver;

use crate::{
    engine::{Clip, Project, Track, normalize_import},
    library::Library,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_BYTES: usize = 16 * 1024 * 1024;
// Guards against a file that is not a song. A valid song stays far below it.
const MAX_SONG_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_SOURCE_TICKS: u32 = 999 * 32;
const PROCESSORS: [&str; 10] = [
    "clip", "tube", "subeq", "comp", "max", "addeq", "tape", "width", "mbc", "lim",
];

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    format_version: u32,
    app_version: String,
    created_at: String,
    modified_at: String,
    song: Song,
    lanes: Vec<Lane>,
    fx_buses: Vec<FxBus>,
    master_bus: MasterBus,
    #[serde(default)]
    generator: Option<Value>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Song {
    bpm: f64,
    master_gain: f32,
    clip_edge_micro_fades: Fades,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Fades {
    enabled: bool,
    fade_in_ms: f64,
    fade_out_ms: f64,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lane {
    id: String,
    name: String,
    gain: f32,
    muted: bool,
    solo: bool,
    pan: f32,
    #[serde(default)]
    stereo_pair_id: Option<String>,
    sends: [f32; 4],
    placements: Vec<Placement>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Placement {
    id: String,
    sample_ref: String,
    sample_name: String,
    #[serde(rename = "nativeBPM")]
    native_bpm: Option<f64>,
    start_tick: u32,
    duration_ticks: u32,
    duration_seconds: Option<f64>,
    slot: Option<u8>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FxBus {
    id: String,
    index: usize,
    name: String,
    module: Value,
    powered: bool,
    return_level: f64,
    limiter_enabled: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MasterBus {
    order: Vec<String>,
    power: HashMap<String, bool>,
    params: HashMap<String, f64>,
    preset: Option<String>,
}

/// The fields that both format versions share, read before the strict parse.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Header {
    format_version: u32,
    created_at: String,
}

fn read(path: &Path) -> Result<(Header, Vec<u8>), String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| format!("Cannot open the MixJam project: {e}"))?
        .take(MAX_SONG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_SONG_BYTES {
        return Err("The MixJam project exceeds the 1 GiB limit.".into());
    }
    let header = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read the MixJam project: {e}"))?;
    Ok((header, bytes))
}

pub fn load(path: &Path, library: &Library) -> Result<Project, String> {
    load_song(path, library).map(|(project, _)| project)
}

/// Open a version 8 or 9 song or import a version 7 project. The flag is true for
/// a song, which Save can replace; a version 7 project is an import and stays
/// unchanged. The file is read once.
pub fn load_song(path: &Path, library: &Library) -> Result<(Project, bool), String> {
    let (header, bytes) = read(path)?;
    match header.format_version {
        8 | song::FORMAT_VERSION => song::parse(&bytes).map(|project| (project, true)),
        7 => {
            if bytes.len() > MAX_BYTES {
                return Err("The MixJam project exceeds the 16 MiB limit.".into());
            }
            let document: Document = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Cannot read the MixJam project: {e}"))?;
            convert(document, path, library).map(|project| (project, false))
        }
        _ => Err("Only MixJam format versions 7, 8, and 9 are supported.".into()),
    }
}

/// Write the song as a format version 9 document.
pub fn save(project: &Project, path: &Path) -> Result<(), String> {
    song::save(project, path)
}

fn text(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}
fn range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && value >= min && value <= max
}
fn relative_wave(value: &str) -> bool {
    text(value)
        && !value.contains(['\\', ':'])
        && !value.starts_with('/')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(value)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
}

fn validate(document: &Document) -> Result<(), String> {
    if document.format_version != 7 {
        return Err("Only MixJam format version 7 is supported.".into());
    }
    if !text(&document.app_version) || !text(&document.created_at) || !text(&document.modified_at) {
        return Err("The MixJam document metadata is invalid.".into());
    }
    if !(1..=64).contains(&document.lanes.len())
        || !range(document.song.bpm, 50.0, 200.0)
        || !range(f64::from(document.song.master_gain), 0.0, 1.0)
    {
        return Err("The MixJam track count, tempo, or master gain is invalid.".into());
    }
    let fades = &document.song.clip_edge_micro_fades;
    if !range(fades.fade_in_ms, 0.0, 20.0) || !range(fades.fade_out_ms, 0.0, 20.0) {
        return Err("The MixJam clip edge fade settings are invalid.".into());
    }
    let master = &document.master_bus;
    let order: HashSet<_> = master.order.iter().map(String::as_str).collect();
    if master.order.len() != 10
        || master.power.len() != 10
        || PROCESSORS
            .iter()
            .any(|id| !order.contains(id) || !master.power.contains_key(*id))
        || master.params.values().any(|v| !v.is_finite())
        || master
            .preset
            .as_ref()
            .is_some_and(|v| !["Cheat Sheet", "Gentle", "Loud", "Bypass All"].contains(&v.as_str()))
    {
        return Err("The MixJam master bus settings are invalid.".into());
    }
    if master
        .params
        .get("gain.trim")
        .is_none_or(|v| !range(*v, -24.0, 24.0))
    {
        return Err("The MixJam master gain trim is invalid.".into());
    }
    if document.fx_buses.len() != 4 {
        return Err("The MixJam project must contain four return buses.".into());
    }
    for (index, bus) in document.fx_buses.iter().enumerate() {
        if bus.index != index
            || bus.id != format!("fx-{}", index + 1)
            || bus.name != format!("FX{}", index + 1)
            || !range(bus.return_level, 0.0, 1.0)
            || !bus.module.is_object()
            || bus.module.get("type").and_then(Value::as_str).is_none()
        {
            return Err("The MixJam return bus settings are invalid.".into());
        }
    }
    let mut lane_ids = HashSet::new();
    let mut placement_ids = HashSet::new();
    let mut durations = HashMap::new();
    let mut count = 0usize;
    for lane in &document.lanes {
        if !text(&lane.id)
            || !text(&lane.name)
            || !lane_ids.insert(&lane.id)
            || !range(f64::from(lane.gain), 0.0, 1.0)
            || !range(f64::from(lane.pan), -1.0, 1.0)
            || lane.sends.iter().any(|v| !range(f64::from(*v), 0.0, 1.0))
            || lane.stereo_pair_id.as_ref().is_some_and(|v| !text(v))
        {
            return Err("The MixJam lane identity or mixer settings are invalid.".into());
        }
        for p in &lane.placements {
            count += 1;
            if count > 100_000 {
                return Err("The MixJam project exceeds the 100,000-clip limit.".into());
            }
            if !text(&p.id)
                || !placement_ids.insert(&p.id)
                || !text(&p.sample_name)
                || !relative_wave(&p.sample_ref)
                || p.duration_ticks == 0
                || p.start_tick
                    .checked_add(p.duration_ticks)
                    .is_none_or(|end| end > MAX_SOURCE_TICKS)
                || p.native_bpm.is_some_and(|v| !v.is_finite() || v <= 0.0)
                || p.duration_seconds
                    .is_some_and(|v| !v.is_finite() || v <= 0.0)
                || p.slot.is_some_and(|v| v > 8)
            {
                return Err(
                    "A MixJam placement has invalid identity, WAV path, timing, or metadata."
                        .into(),
                );
            }
            if durations
                .insert(&p.sample_ref, p.duration_ticks)
                .is_some_and(|old| old != p.duration_ticks)
            {
                return Err("MixJam placements for one WAV must use the same source span.".into());
            }
        }
    }
    Ok(())
}

fn convert(document: Document, path: &Path, library: &Library) -> Result<Project, String> {
    validate(&document)?;
    let mut project = Project {
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        bpm: document.song.bpm,
        master_gain: document.song.master_gain,
        ..Project::default()
    };
    project.source_notices.push("Imported MixJam v7 arrangement. MixJam master processing, gain trim, return effects, and clip edge fades are not rendered. The sound can differ. Saved source controls remain in the next notice. Mute and solo become channel routing; later lane triggers cut earlier clips.".into());
    project.source_notices.push(format!("MixJam v7 source controls: {}", serde_json::json!({
        "appVersion":document.app_version, "createdAt":document.created_at, "modifiedAt":document.modified_at,
        "masterBus":document.master_bus, "fxBuses":document.fx_buses,
        "clipEdgeMicroFades":document.song.clip_edge_micro_fades, "generator":document.generator,
        "lanes":document.lanes.iter().map(|lane| serde_json::json!({"id":lane.id,"name":lane.name,
            "muted":lane.muted,"solo":lane.solo,"stereoPairId":lane.stereo_pair_id,"sends":lane.sends,
            "placements":lane.placements})).collect::<Vec<_>>()
    })));
    let solo = document.lanes.iter().any(|lane| lane.solo);
    let mut references = ReferenceResolver::new(library);
    let mut imported_paths: HashMap<(String, u32), u32> = HashMap::new();
    for (track, mut lane) in document.lanes.into_iter().enumerate() {
        let audible = if solo { lane.solo } else { !lane.muted };
        project.tracks[track] = Track {
            gain: lane.gain,
            polarity_inverted: false,
            pan: lane.pan,
            left_enabled: audible,
            right_enabled: audible,
        };
        // The format reader sorts equal-time placements by ID. MixJam gives the
        // last trigger at each start tick precedence and cuts the previous voice.
        lane.placements
            .sort_by(|a, b| a.start_tick.cmp(&b.start_tick).then(a.id.cmp(&b.id)));
        for (index, placement) in lane.placements.iter().enumerate() {
            let candidates = references.resolve_candidates(&placement.sample_ref);
            let resolved = candidates
                .as_ref()
                .map(|paths| paths[0].clone())
                .map_err(Clone::clone);
            let source = resolved
                .as_ref()
                .cloned()
                .unwrap_or_else(|_| PathBuf::from(&placement.sample_ref));
            let key = (
                placement.sample_ref.to_lowercase(),
                placement.duration_ticks,
            );
            let sample_id = if let Some(id) = imported_paths.get(&key) {
                *id
            } else {
                // Only the duration is needed here; the renderer decodes the audio.
                let wave =
                    resolved.and_then(|source| crate::audio::visit_wav_samples(&source, |_, _| {}));
                let tempo = match &wave {
                    Ok((spec, frames)) if *frames > 0 => {
                        f64::from(placement.duration_ticks) * 240.0
                            / 32.0
                            / (f64::from(*frames) / f64::from(spec.sample_rate))
                    }
                    _ => placement
                        .duration_seconds
                        .map(|seconds| f64::from(placement.duration_ticks) * 240.0 / 32.0 / seconds)
                        .unwrap_or_else(|| placement.native_bpm.unwrap_or(document.song.bpm)),
                };
                // A second reference may give the same WAV a different span.
                // Imported project identities must keep those rates independent.
                let id = crate::products::IMPORTED_ID_START
                    .checked_add(
                        (project.imported_samples.len() + project.missing_samples.len()) as u32,
                    )
                    .ok_or("The MixJam imported sample ID limit was reached.")?;
                let imported = crate::engine::ImportedSample {
                    id,
                    name: placement.sample_name.trim().to_string(),
                    group_name: "MixJam".into(),
                    path: source.clone(),
                    source_bpm: tempo,
                    source_sample_rate: wave.as_ref().ok().map(|(spec, _)| spec.sample_rate),
                    origin: None,
                };
                crate::engine::validate_import_metadata(&imported)?;
                match wave {
                    Ok((_, frames)) if frames > 0 => project.imported_samples.push(imported),
                    result => {
                        let reason = result
                            .err()
                            .unwrap_or_else(|| "The source WAV has no audio frames.".into());
                        let mut missing = crate::engine::MissingSample::external(
                            &imported,
                            placement.duration_ticks * 8,
                            reason,
                        );
                        missing.length_inferred = false;
                        missing
                            .metadata
                            .insert("Playback timing policy".into(), "mixjam-span".into());
                        missing.metadata.insert(
                            "MixJam source reference".into(),
                            placement.sample_ref.clone(),
                        );
                        missing.metadata.insert(
                            "MixJam source metadata".into(),
                            serde_json::to_string(placement).map_err(|error| error.to_string())?,
                        );
                        project.missing_samples.push(missing);
                    }
                }
                imported_paths.insert(key, id);
                id
            };
            if let Ok(candidates) = candidates
                && candidates.len() > 1
            {
                crate::library::record_wav_conflict(
                    &mut project,
                    &placement.sample_ref,
                    sample_id,
                    candidates,
                );
                if let Some(conflict) = project
                    .sample_conflicts
                    .iter_mut()
                    .find(|conflict| conflict.sample_ids.contains(&sample_id))
                {
                    conflict
                        .source_spans
                        .insert(sample_id, placement.duration_ticks * 8);
                }
            }
            let length = lane
                .placements
                .get(index + 1)
                .map_or(placement.duration_ticks, |next| {
                    placement
                        .duration_ticks
                        .min(next.start_tick - placement.start_tick)
                });
            if length == 0 {
                continue;
            }
            project.clips.push(Clip {
                id: project.clips.len() as u64 + 1,
                sample_id,
                track,
                start_tick: placement.start_tick * 8,
                length_ticks: length * 8,
                offset_frames: 0,
                gain: 1.0,
            });
        }
    }
    normalize_import(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Value {
        json!({"formatVersion":7,"appVersion":"1.0","createdAt":"2026-01-01T00:00:00Z","modifiedAt":"2026-01-01T00:00:00Z",
            "song":{"bpm":127,"masterGain":0.8,"clipEdgeMicroFades":{"enabled":false,"fadeInMs":2,"fadeOutMs":4}},
            "lanes":[{"id":"lane-1","name":"Drums","gain":0.6,"muted":true,"solo":true,"pan":-0.25,"sends":[0,0,0,0],"placements":[
                {"id":"a","sampleRef":"kick.wav","sampleName":"Kick","nativeBPM":140,"startTick":4,"durationTicks":32,"durationSeconds":2,"slot":null},
                {"id":"b","sampleRef":"kick.wav","sampleName":"Kick","nativeBPM":140,"startTick":20,"durationTicks":32,"durationSeconds":2,"slot":null}]}],
            "fxBuses":(0..4).map(|i| json!({"id":format!("fx-{}",i+1),"index":i,"name":format!("FX{}",i+1),"module":{"type":"empty"},"powered":true,"returnLevel":1,"limiterEnabled":true})).collect::<Vec<_>>(),
            "masterBus":{"order":PROCESSORS,"power":PROCESSORS.iter().map(|id| (id.to_string(),false)).collect::<HashMap<_,_>>(),"params":{"gain.trim":0},"preset":"Bypass All"}})
    }
    #[test]
    fn rejects_invalid_settings_and_unsafe_paths_before_audio_resolution() {
        for (pointer, value, expected) in [
            (
                "/song/clipEdgeMicroFades/fadeInMs",
                json!(21),
                "fade settings",
            ),
            ("/masterBus/params/gain.trim", json!(25), "gain trim"),
            (
                "/lanes/0/placements/0/sampleRef",
                json!("../kick.wav"),
                "invalid identity",
            ),
            (
                "/lanes/0/placements/1/durationTicks",
                json!(33),
                "same source span",
            ),
            ("/formatVersion", json!(8), "version 7"),
        ] {
            let mut value_doc = fixture();
            *value_doc.pointer_mut(pointer).unwrap() = value;
            let document = serde_json::from_value(value_doc).unwrap();
            assert!(
                convert(document, Path::new("song.mixjam"), &Library::empty())
                    .unwrap_err()
                    .contains(expected),
                "{pointer}"
            );
        }
    }
    #[test]
    fn wav_only_import_converts_clock_solo_and_monophonic_spans() {
        let root = std::env::temp_dir().join(format!("mixjam-import-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let wave = root.join("kick.wav");
        let mut writer = hound::WavWriter::create(
            &wave,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..16000 {
            writer.write_sample(1000i16).unwrap();
        }
        writer.finalize().unwrap();
        let mut library = Library::empty();
        library.import_wav(&wave, None, "Drum").unwrap();
        let project = convert(
            serde_json::from_value(fixture()).unwrap(),
            Path::new("song.mixjam"),
            &library,
        )
        .unwrap();
        assert_eq!(project.bpm, 120.0);
        assert_eq!(project.import_notices[0].source_bpm, 127.0);
        assert_eq!(project.clips.len(), 2);
        assert_eq!(project.clips[0].start_tick, 32);
        assert_eq!(project.clips[0].length_ticks, 128);
        assert_eq!(project.clips[1].length_ticks, 256);
        assert_eq!(project.imported_samples.len(), 1);
        assert_eq!(project.imported_samples[0].source_bpm, 120.0);
        assert!(project.tracks[0].left_enabled);
        assert_eq!(project.tracks[0].pan, -0.25);
        assert_eq!(project.tracks[0].gain, 0.6);
        assert!(project.libraries.is_empty());
        let mut active = fixture();
        active["song"]["clipEdgeMicroFades"]["enabled"] = json!(true);
        active["masterBus"]["power"]["lim"] = json!(true);
        active["fxBuses"][0]["module"]["type"] = json!("reverb");
        active["lanes"][0]["placements"][1]["sampleRef"] = json!("relocated/kick.wav");
        active["lanes"][0]["placements"][1]["durationTicks"] = json!(64);
        let projected = convert(
            serde_json::from_value(active).unwrap(),
            Path::new("song.mixjam"),
            &library,
        )
        .unwrap();
        assert_eq!(projected.imported_samples.len(), 2);
        assert_ne!(projected.clips[0].sample_id, projected.clips[1].sample_id);
        assert_eq!(projected.imported_samples[1].source_bpm, 240.0);
        assert!(projected.source_notices[0].contains("not rendered"));
        assert!(projected.source_notices[1].contains("\"lim\":true"));
        assert!(projected.source_notices[1].contains("\"type\":\"reverb\""));
        fs::remove_file(wave).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn legacy_folder_recovery_survives_save_reopen_and_resolves_saved_placeholders() {
        let root =
            std::env::temp_dir().join(format!("mixjam-legacy-layout-{}", std::process::id()));
        let selected = root.join("_original");
        let legacy = root.join("House/Drum/kick.wav");
        fs::create_dir_all(&selected).unwrap();
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        let write = |path: &Path, frames| {
            let mut writer = hound::WavWriter::create(
                path,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 8000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for _ in 0..frames {
                writer.write_sample(1000_i16).unwrap();
            }
            writer.finalize().unwrap();
        };
        write(&legacy, 16000);
        // A same-basename palette sound must not replace the full saved reference.
        write(&selected.join("kick.wav"), 8000);
        let library = Library::open_wav_folder(&selected, 140.0).unwrap();
        let mut source = fixture();
        for placement in source["lanes"][0]["placements"].as_array_mut().unwrap() {
            placement["sampleRef"] = json!("House/Drum/kick.wav");
        }
        let input = root.join("old.mixjam");
        fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
        let (opened, _) = crate::project_io::open(&input, &library).unwrap();
        assert!(opened.missing_samples.is_empty());
        let id = opened.clips[0].sample_id;
        let audio = opened
            .imported_samples
            .iter()
            .find(|sample| sample.id == id)
            .unwrap();
        assert_eq!(audio.path, fs::canonicalize(&legacy).unwrap());
        assert_eq!(audio.source_bpm, 120.0);
        assert_eq!(opened.clips[1].sample_id, id);

        let saved = root.join("saved.mixjam");
        crate::project_io::save(&opened, &saved).unwrap();
        let (reopened, _) = crate::project_io::open(&saved, &library).unwrap();
        assert_eq!(reopened.clips, opened.clips);
        assert_eq!(
            reopened
                .imported_samples
                .iter()
                .find(|sample| sample.id == id)
                .unwrap()
                .path,
            audio.path
        );

        let (missing, _) = crate::project_io::open(&input, &Library::empty()).unwrap();
        assert_eq!(missing.missing_samples.len(), 1);
        crate::project_io::save(&missing, &saved).unwrap();
        let (restored, _) = crate::project_io::open(&saved, &library).unwrap();
        assert!(restored.missing_samples.is_empty());
        assert_eq!(restored.clips, missing.clips);
        let restored_audio = restored
            .imported_samples
            .iter()
            .find(|sample| sample.id == restored.clips[0].sample_id)
            .unwrap();
        assert_eq!(restored_audio.path, audio.path);
        assert_eq!(restored_audio.source_bpm, 120.0);

        // A moved reference with two matches opens audibly with a conflict choice.
        for placement in source["lanes"][0]["placements"].as_array_mut().unwrap() {
            placement["sampleRef"] = json!("Old/kick.wav");
        }
        fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
        let (conflicted, conflict_library) = crate::project_io::open(&input, &library).unwrap();
        assert!(conflicted.missing_samples.is_empty());
        assert_eq!(conflicted.sample_conflicts.len(), 1);
        let conflict = &conflicted.sample_conflicts[0];
        assert_eq!(conflict.candidates.len(), 2);
        assert_eq!(conflict.source_spans[&conflicted.clips[0].sample_id], 256);
        assert!(!conflict.confirmed);
        let before_clips = conflicted.clips.clone();
        let mut session =
            crate::application::Session::new(std::sync::Arc::new(conflict_library), conflicted)
                .unwrap();
        session.resolve_sample_conflict(0, 1).unwrap();
        assert_eq!(session.project().clips, before_clips);
        let selected_id = session.project().clips[0].sample_id;
        let selected_audio = session
            .project()
            .imported_samples
            .iter()
            .find(|sample| sample.id == selected_id)
            .unwrap();
        assert_eq!(selected_audio.source_bpm, 240.0);
        assert!(session.sample_conflicts()[0].confirmed);
        crate::project_io::save(session.project(), &saved).unwrap();
        let (reopened, _) = crate::project_io::open(&saved, &library).unwrap();
        assert_eq!(
            reopened.sample_conflicts,
            session.project().sample_conflicts
        );
        assert_eq!(
            reopened
                .imported_samples
                .iter()
                .find(|sample| sample.id == selected_id)
                .unwrap()
                .path,
            selected_audio.path
        );
        fs::remove_dir_all(root).unwrap();
    }
}
