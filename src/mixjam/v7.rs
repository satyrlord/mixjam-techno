//! The preserved MixJam JSON v7 format: its records, the validation of a
//! file before any audio is resolved, and the conversion into a project.

use super::{ReferenceResolver, range, relative_wave, text};
use crate::{
    engine::{Clip, Project, Track, normalize_import},
    library::Library,
    master::{Knob, MasterChain, PedalKind},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

/// Where a placement ends at the latest, in ticks of the earlier song format:
/// that format's timeline is 999 bars of 32 ticks. It is the format's own
/// limit, not a choice; the current timeline is far longer.
const MAX_SOURCE_TICKS: u32 = 999 * 32;

pub(super) const PROCESSORS: [&str; 10] = [
    "clip", "tube", "subeq", "comp", "max", "addeq", "tape", "width", "mbc", "lim",
];

/// The pedal of each MixJam master processor, in `PROCESSORS` order.
const PEDALS: [PedalKind; 10] = [
    PedalKind::SoftClip,
    PedalKind::Tube,
    PedalKind::TrimEq,
    PedalKind::BusCompressor,
    PedalKind::Maximizer,
    PedalKind::LiftEq,
    PedalKind::Tape,
    PedalKind::Imaging,
    PedalKind::Multiband,
    PedalKind::Limiter,
];

/// The knob of each MixJam master parameter. `tape.ips`, the tape speed
/// switch, is read on its own.
const PARAMETERS: [(&str, Knob); 23] = [
    ("gain.trim", Knob::Trim),
    ("clip.amount", Knob::SoftClipAmount),
    ("clip.ceil", Knob::SoftClipCeiling),
    ("tube.drive", Knob::TubeDrive),
    ("tube.mix", Knob::TubeMix),
    ("subeq.hp", Knob::TrimEqHighPass),
    ("subeq.mud", Knob::TrimEqMud),
    ("subeq.harsh", Knob::TrimEqHarsh),
    ("comp.thr", Knob::CompressorThreshold),
    ("comp.ratio", Knob::CompressorRatio),
    ("comp.att", Knob::CompressorAttack),
    ("comp.rel", Knob::CompressorRelease),
    ("max.boost", Knob::MaximizerBoost),
    ("addeq.low", Knob::LiftLow),
    ("addeq.air", Knob::LiftAir),
    ("tape.drive", Knob::TapeDrive),
    ("width.width", Knob::ImagingWidth),
    ("width.mono", Knob::ImagingMonoBelow),
    ("mbc.lo", Knob::MultibandLow),
    ("mbc.mid", Knob::MultibandMid),
    ("mbc.hi", Knob::MultibandHigh),
    ("lim.gain", Knob::LimiterGain),
    ("lim.ceil", Knob::LimiterCeiling),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Document {
    pub(super) format_version: u32,
    app_version: String,
    pub(super) created_at: String,
    modified_at: String,
    song: Song,
    lanes: Vec<Lane>,
    fx_buses: Vec<FxBus>,
    master_bus: MasterBus,
    #[serde(default)]
    generator: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Song {
    pub(super) bpm: f64,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lane {
    pub(super) id: String,
    name: String,
    pub(super) gain: f32,
    pub(super) muted: bool,
    pub(super) solo: bool,
    pub(super) pan: f32,
    #[serde(default)]
    stereo_pair_id: Option<String>,
    pub(super) sends: [f32; 4],
    placements: Vec<Placement>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Placement {
    pub(super) id: String,
    sample_ref: String,
    sample_name: String,
    #[serde(rename = "nativeBPM")]
    native_bpm: Option<f64>,
    pub(super) start_tick: u32,
    duration_ticks: u32,
    duration_seconds: Option<f64>,
    slot: Option<u8>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FxBus {
    pub(super) id: String,
    index: usize,
    name: String,
    pub(super) module: Value,
    pub(super) powered: bool,
    pub(super) return_level: f64,
    limiter_enabled: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MasterBus {
    pub(super) order: Vec<String>,
    pub(super) power: HashMap<String, bool>,
    params: HashMap<String, f64>,
    preset: Option<String>,
}

pub(super) fn validate(document: &Document) -> Result<(), String> {
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
    if master.params.iter().any(|(id, value)| match id.as_str() {
        "tape.ips" => *value != 0.0 && *value != 1.0,
        // This decimal endpoint is not exact in f32. Validate the source's
        // f64 value against its own range before conversion to a pedal setting.
        "comp.att" => !range(*value, 0.1, 30.0),
        id => PARAMETERS
            .iter()
            .find(|(name, _)| *name == id)
            .is_none_or(|(_, knob)| {
                let limits = knob.range();
                !range(*value, f64::from(limits.low), f64::from(limits.high))
            }),
    }) {
        return Err("The MixJam master bus settings are invalid.".into());
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
            if count > crate::engine::MAX_CLIPS {
                return Err("The MixJam project exceeds the 1,000,000-clip limit.".into());
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

/// The master pedalboard for a validated MixJam master bus: the Booster pedals
/// off at the front, then the mastering pedals in the bus's order with its
/// switches and settings, except that the limiter always comes last. A
/// parameter the file leaves out keeps its default.
fn master_from_bus(bus: &MasterBus) -> MasterChain {
    let mut chain = MasterChain::default();
    let pedal = |id: &str| PROCESSORS.iter().position(|p| *p == id).map(|i| PEDALS[i]);
    let movable = bus
        .order
        .iter()
        .filter_map(|id| pedal(id))
        .filter(|kind| kind.movable());
    for (slot, kind) in chain.order[3..12].iter_mut().zip(movable) {
        *slot = kind;
    }
    for (id, on) in &bus.power {
        if let Some(kind) = pedal(id) {
            chain.set_on(kind, *on);
        }
    }
    for (id, knob) in PARAMETERS {
        if let Some(value) = bus.params.get(id) {
            knob.set(&mut chain, *value as f32);
        }
    }
    if let Some(ips) = bus.params.get("tape.ips") {
        chain.tape.speed = if *ips == 0.0 { 15 } else { 30 };
    }
    chain
}

/// The FX buses for validated MixJam return buses. A module of a type this
/// application does not have imports as an empty slot.
fn fx_from_buses(buses: &[FxBus]) -> Result<crate::fx::FxBuses, String> {
    let mut fx = crate::fx::FxBuses::default();
    for (slot, bus) in fx.buses.iter_mut().zip(buses) {
        let mut module = bus.module.clone();
        if let Some(fields) = module.as_object_mut() {
            fields.remove("id");
        }
        let known = matches!(
            module.get("type").and_then(Value::as_str),
            Some("empty" | "echoform-delay" | "aetherform-reverb")
        );
        *slot = crate::fx::FxBus {
            module: if known {
                serde_json::from_value(module)
                    .map_err(|error| format!("A MixJam return module is invalid: {error}"))?
            } else {
                crate::fx::Module::Empty
            },
            powered: bus.powered,
            return_level: bus.return_level as f32,
            limiter: bus.limiter_enabled,
        };
    }
    fx.validate()?;
    Ok(fx)
}

pub(super) fn convert(
    document: Document,
    path: &Path,
    library: &Library,
) -> Result<Project, String> {
    validate(&document)?;
    let fx = fx_from_buses(&document.fx_buses)?;
    let mut project = Project {
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        bpm: document.song.bpm,
        master_gain: document.song.master_gain,
        master: master_from_bus(&document.master_bus),
        fx,
        ..Project::default()
    };
    project.source_notices.push(
        "Imported MixJam v7 arrangement. The master bus plays through this application's \
         mastering pedals with its saved order, switches, and settings; they are new designs, so \
         the sound can differ. The return buses and lane sends play through this application's \
         Echoform Delay and Aetherform Reverb, new designs with the same controls; another module \
         type imports as an empty slot. Clip edge fades are not rendered. Saved source controls \
         remain in the next notice. A soloed lane imports unmuted, because MixJam plays a soloed \
         lane even when it is muted; later lane triggers cut earlier clips."
            .into(),
    );
    project.source_notices.push(format!(
        "MixJam v7 source controls: {}",
        serde_json::json!({
            "appVersion":document.app_version, "createdAt":document.created_at,
            "modifiedAt":document.modified_at,
            "masterBus":document.master_bus, "fxBuses":document.fx_buses,
            "clipEdgeMicroFades":document.song.clip_edge_micro_fades,
            "generator":document.generator,
            "lanes":document.lanes.iter().map(|lane| serde_json::json!({
                "id":lane.id,"name":lane.name,
                "muted":lane.muted,"solo":lane.solo,"stereoPairId":lane.stereo_pair_id,
                "sends":lane.sends,"placements":lane.placements})).collect::<Vec<_>>()
        })
    ));
    let mut references = ReferenceResolver::new(library);
    let mut imported_paths: HashMap<(String, u32), u32> = HashMap::new();
    for (track, mut lane) in document.lanes.into_iter().enumerate() {
        project.tracks[track] = Track {
            gain: lane.gain,
            polarity_inverted: false,
            pan: lane.pan,
            muted: lane.muted && !lane.solo,
            solo: lane.solo,
            sends: lane.sends,
            ..Track::default()
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
                let id = project.next_import_id()?;
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
                fades: Default::default(),
                pitch_cents: 0,
                reverse: false,
            });
        }
    }
    normalize_import(project)
}
