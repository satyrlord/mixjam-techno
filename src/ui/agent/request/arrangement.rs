//! Agent requests that edit the arrangement: placing, moving, deleting, and
//! selecting clips, the playhead, the loop, sections, automation, and one
//! clip's fields.
use super::*;

impl MixJamApp {
    /// Apply an arrangement request through the session.
    pub(super) fn agent_arrangement(&mut self, request: Request) -> Result<Start, String> {
        Ok(match request {
            Request::Place {
                sample,
                track,
                bar,
                tick,
            } => {
                let sample = self.agent_sample(&sample)?;
                if !(1..=TRACK_COUNT).contains(&track) {
                    return Err(format!("Tracks are 1 to {TRACK_COUNT}."));
                }
                let id = self.try_place(sample, track - 1, wire::position_tick(bar, tick)?)?;
                Start::Run {
                    steps: vec![Step::Idle],
                    clips: vec![id],
                    control: None,
                }
            }
            Request::MoveClips {
                clips,
                tracks,
                bars,
                ticks,
                copy,
            } => {
                let bars = (bars * f64::from(TICKS_PER_BAR)).round();
                if !bars.is_finite() || bars.abs() > f64::from(u32::MAX) {
                    return Err("The bar distance is out of range.".into());
                }
                let ticks = ticks
                    .checked_add(bars as i64)
                    .ok_or("The combined tick distance is out of range.")?;
                let clips = self.try_move_clips(&clips, tracks, ticks, copy)?;
                Start::Run {
                    steps: vec![Step::Idle],
                    clips,
                    control: None,
                }
            }
            Request::DeleteClips { clips } => {
                self.session.edit(EditCommand::DeleteClips(clips))?;
                self.reconcile_selection();
                self.sync_notice();
                Start::run(vec![Step::Idle])
            }
            Request::Select { clips, sample } => {
                let sample = sample
                    .map(|sample| self.agent_sample(&sample))
                    .transpose()?;
                let project = self.session.project();
                if let Some(id) = clips
                    .iter()
                    .find(|id| !project.clips.iter().any(|clip| clip.id == **id))
                {
                    return Err(format!("Clip {id} does not exist."));
                }
                let first = clips
                    .first()
                    .and_then(|id| project.clips.iter().find(|clip| clip.id == *id));
                self.selection.sample = sample.or(first.map(|clip| clip.sample_id));
                self.selection.clip = clips.first().copied();
                self.selection.clips = clips.into_iter().collect();
                Start::run(vec![Step::Idle])
            }
            Request::Seek { bar, tick } => {
                self.seek(wire::position_tick(bar, tick)?);
                Start::run(vec![Step::Idle])
            }
            Request::SetLoop {
                first_bar,
                last_bar,
            } => {
                let region = loop_region(first_bar, last_bar)?;
                self.session.edit(EditCommand::SetLoop(region))?;
                self.sync_notice();
                Start::run(vec![Step::Idle])
            }
            Request::SetSections { sections } => {
                let sections = sections
                    .into_iter()
                    .map(|section| {
                        Ok(mixjam_techno::application::Section {
                            start_tick: wire::position_tick(section.bar, section.tick)?,
                            name: section.name,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                self.session.edit(EditCommand::SetSections(sections))?;
                self.cancel_section_edit();
                self.sync_notice();
                Start::run(vec![Step::Idle])
            }
            Request::Automate {
                track,
                bus,
                lane,
                points,
            } => {
                let command = automation_command(track, bus, &lane, points)?;
                self.session.edit(command)?;
                self.sync_notice();
                Start::run(vec![Step::Idle])
            }
            request @ Request::Clip { clip: id, .. } => {
                let clip = self
                    .session
                    .project()
                    .clips
                    .iter()
                    .find(|clip| clip.id == id)
                    .ok_or(format!("Clip {id} does not exist."))?;
                self.session.edit_discrete(clip_edit(clip, request)?)?;
                self.sync_notice();
                Start::Run {
                    steps: vec![Step::Idle],
                    clips: vec![id],
                    control: None,
                }
            }
            _ => return Err("That is not an arrangement request.".into()),
        })
    }
}

/// The loop region of a `set_loop` request: both bars from 1, or neither for no loop.
fn loop_region(first_bar: Option<u32>, last_bar: Option<u32>) -> Result<Option<[u32; 2]>, String> {
    match (first_bar, last_bar) {
        (None, None) => Ok(None),
        (Some(first), Some(last)) if (1..=last).contains(&first) => Ok(Some([
            (first - 1).saturating_mul(TICKS_PER_BAR),
            last.saturating_mul(TICKS_PER_BAR),
        ])),
        _ => Err("Give first_bar and last_bar from 1, or neither to turn the loop off.".into()),
    }
}

/// The edit of an `automate` request, on one track lane or one FX bus return.
fn automation_command(
    track: Option<usize>,
    bus: Option<usize>,
    lane: &str,
    points: Vec<[f64; 2]>,
) -> Result<EditCommand, String> {
    let points = points
        .into_iter()
        .map(|[bar, value]| {
            Ok(automation::Point {
                tick: wire::position_tick(Some(bar), None)?,
                value: value as f32,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(match (track, bus) {
        (Some(track), None) => {
            if !(1..=TRACK_COUNT).contains(&track) {
                return Err(format!("Tracks are 1 to {TRACK_COUNT}."));
            }
            let lane = automation::Lane::from_key(lane).ok_or(
                "Track lanes are volume, send1, send2, send3, send4, filter, pan, repeat, \
                 repeatgate, and pitch.",
            )?;
            EditCommand::SetAutomation {
                track: track - 1,
                lane,
                points,
            }
        }
        (None, Some(bus)) => {
            if !(1..=mixjam_techno::application::fx::BUSES).contains(&bus) {
                return Err(format!(
                    "FX buses are 1 to {}.",
                    mixjam_techno::application::fx::BUSES
                ));
            }
            if lane != "return" {
                return Err("An FX bus has one lane: return.".into());
            }
            EditCommand::SetReturnAutomation {
                bus: bus - 1,
                points,
            }
        }
        _ => return Err("Give either track= or bus=.".into()),
    })
}
