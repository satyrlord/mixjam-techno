//! Continuous control gestures: quick edits of one control form one undo step.
use super::{EditCommand, Knob, Project, Session, TRACK_COUNT, TrackSwitch};

/// One continuously adjustable control. Edits of the same control in quick
/// succession, such as the frames of a slider drag, form one undo step.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Gesture {
    Tempo,
    Master,
    Track(usize, TrackControl),
    Clip(u64, ClipControl),
    /// One automation lane, such as the frames of a point drag.
    Automation(usize, crate::automation::Lane),
    /// One FX bus's return lane.
    Return(usize),
    /// The trim or one knob of the master pedalboard.
    Pedal(Knob),
    Fx(usize, FxControl),
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum TrackControl {
    Gain,
    Pan,
    Send(usize),
    Eq(crate::eq::EqParam),
    Drive(crate::drive::DriveParam),
    Compressor(crate::compressor::CompressorParam),
    Width,
    Ducker(crate::ducker::DuckerParam),
    DuckerKey,
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum FxControl {
    Return,
    Delay(crate::fx::DelayParam),
    Reverb(crate::fx::ReverbParam),
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ClipControl {
    Start,
    Gain,
    Trim,
    Length,
    FadeIn,
    FadeOut,
    Pitch,
}
pub(super) const GESTURE_GAP: std::time::Duration = std::time::Duration::from_millis(400);

impl Gesture {
    /// The control a command adjusts, when it adjusts exactly one.
    pub(super) fn of(command: &EditCommand, project: &Project) -> Option<Self> {
        match command {
            EditCommand::SetTempo(_) => Some(Self::Tempo),
            EditCommand::SetClip {
                id,
                start_tick,
                gain,
                offset_frames,
                length_ticks,
                fades,
                pitch_cents,
                reverse,
            } => {
                let clip = project.clips.iter().find(|clip| clip.id == *id)?;
                // A switch is a step of its own.
                if clip.reverse != *reverse {
                    return None;
                }
                let mut controls = Vec::new();
                if clip.start_tick != *start_tick {
                    controls.push(ClipControl::Start);
                }
                if clip.fades.fade_in != fades.fade_in {
                    controls.push(ClipControl::FadeIn);
                }
                if clip.fades.fade_out != fades.fade_out {
                    controls.push(ClipControl::FadeOut);
                }
                if clip.pitch_cents != *pitch_cents {
                    controls.push(ClipControl::Pitch);
                }
                if clip.gain != *gain {
                    controls.push(ClipControl::Gain);
                }
                if clip.offset_frames != *offset_frames {
                    controls.push(ClipControl::Trim);
                }
                if clip.length_ticks != *length_ticks {
                    controls.push(ClipControl::Length);
                }
                match controls[..] {
                    [control] => Some(Self::Clip(*id, control)),
                    _ => None,
                }
            }
            EditCommand::SetMixer {
                tracks,
                master_gain,
            } if tracks.len() == project.tracks.len() => {
                let mut controls = Vec::new();
                for (index, (old, new)) in project.tracks.iter().zip(tracks).enumerate() {
                    Self::track_controls(index, old, new, &mut controls)?;
                }
                if project.master_gain != *master_gain {
                    controls.push(Self::Master);
                }
                match controls[..] {
                    [control] => Some(control),
                    _ => None,
                }
            }
            EditCommand::SetTrack { index, track } => {
                let mut controls = Vec::new();
                Self::track_controls(*index, project.tracks.get(*index)?, track, &mut controls)?;
                match controls[..] {
                    [control] => Some(control),
                    _ => None,
                }
            }
            EditCommand::SetMaster(chain) => project.master.changed_knob(chain).map(Self::Pedal),
            EditCommand::SetEffects(buses) => Self::fx(&project.fx, buses),
            _ => None,
        }
    }

    /// Adds the continuous controls that differ between `old` and `new`, the
    /// track at `index`. A switch is a step of its own, so it gives `None`.
    fn track_controls(
        index: usize,
        old: &crate::engine::Track,
        new: &crate::engine::Track,
        controls: &mut Vec<Self>,
    ) -> Option<()> {
        if old.polarity_inverted != new.polarity_inverted
            || old.automation != new.automation
            || old.eq.on != new.eq.on
            || old.drive.on != new.drive.on
            || old.compressor.on != new.compressor.on
            || old.ducker.on != new.ducker.on
            || TrackSwitch::ALL
                .iter()
                .any(|switch| switch.get(old) != switch.get(new))
        {
            return None;
        }
        if old.gain != new.gain {
            controls.push(Self::Track(index, TrackControl::Gain));
        }
        if old.pan != new.pan {
            controls.push(Self::Track(index, TrackControl::Pan));
        }
        for (bus, (old, new)) in old.sends.iter().zip(new.sends).enumerate() {
            if *old != new {
                controls.push(Self::Track(index, TrackControl::Send(bus)));
            }
        }
        for param in crate::eq::EqParam::ALL {
            if param.get(&old.eq) != param.get(&new.eq) {
                controls.push(Self::Track(index, TrackControl::Eq(param)));
            }
        }
        for param in crate::drive::DriveParam::ALL {
            if param.get(&old.drive) != param.get(&new.drive) {
                controls.push(Self::Track(index, TrackControl::Drive(param)));
            }
        }
        for param in crate::compressor::CompressorParam::ALL {
            if param.get(&old.compressor) != param.get(&new.compressor) {
                controls.push(Self::Track(index, TrackControl::Compressor(param)));
            }
        }
        if old.width != new.width {
            controls.push(Self::Track(index, TrackControl::Width));
        }
        if old.ducker.key != new.ducker.key {
            controls.push(Self::Track(index, TrackControl::DuckerKey));
        }
        for param in crate::ducker::DuckerParam::ALL {
            if param.get(&old.ducker) != param.get(&new.ducker) {
                controls.push(Self::Track(index, TrackControl::Ducker(param)));
            }
        }
        Some(())
    }

    /// Only one continuous FX control can continue a drag. A switch, module
    /// choice, preset, or multiple controls form a separate history step.
    fn fx(before: &crate::fx::FxBuses, after: &crate::fx::FxBuses) -> Option<Self> {
        use crate::fx::{DelayParam, Module, ReverbParam};
        let mut changed = before
            .buses
            .iter()
            .zip(&after.buses)
            .enumerate()
            .filter(|(_, (old, new))| old != new);
        let (index, (old, new)) = changed.next()?;
        if changed.next().is_some() || old.powered != new.powered || old.limiter != new.limiter {
            return None;
        }
        if old.return_level != new.return_level {
            return (old.module == new.module).then_some(Self::Fx(index, FxControl::Return));
        }
        match (old.module, new.module) {
            (Module::EchoformDelay(old), Module::EchoformDelay(new)) => {
                DelayParam::ALL.into_iter().find_map(|param| {
                    let mut candidate = old;
                    param.set(&mut candidate, param.get(&new));
                    (candidate == new).then_some(Self::Fx(index, FxControl::Delay(param)))
                })
            }
            (Module::AetherformReverb(old), Module::AetherformReverb(new)) => {
                ReverbParam::ALL.into_iter().find_map(|param| {
                    let mut candidate = old;
                    param.set(&mut candidate, param.get(&new));
                    (candidate == new).then_some(Self::Fx(index, FxControl::Reverb(param)))
                })
            }
            _ => None,
        }
    }
}

impl Session {
    /// Let a gesture continue through any pause. Gestures end after a pause in
    /// wall-clock time, and test frames under load can take longer than one.
    #[cfg(all(test, feature = "gui"))]
    pub(crate) fn hold_gestures(&mut self) {
        self.gesture_gap = std::time::Duration::MAX;
    }
    /// Follow the pointer for control gestures. A press starts a new undo
    /// step, so a button such as Unity never joins the drag before it. While
    /// a button is held, the gesture continues through any pause, so a drag
    /// held still stays one step. Call it once per frame, before the controls.
    pub fn pointer_gesture(&mut self, pressed: bool, held: bool) {
        if pressed {
            self.gesture = None;
        } else if held && let Some((_, at)) = &mut self.gesture {
            *at = std::time::Instant::now();
        }
    }
    /// Begin one undo step for a point drag, including a point added by the drag.
    pub fn begin_automation_gesture(
        &mut self,
        track: usize,
        lane: crate::automation::Lane,
    ) -> Result<(), String> {
        if track >= TRACK_COUNT {
            return Err("The track is outside the project.".into());
        }
        if matches!(lane, crate::automation::Lane::Send(bus) if bus >= crate::fx::BUSES) {
            return Err("The FX bus does not exist.".into());
        }
        self.gesture = None;
        self.automation_gesture = Some((track, lane));
        Ok(())
    }
    /// Begin one undo step for a point drag on an FX bus's return lane.
    pub fn begin_return_gesture(&mut self, bus: usize) -> Result<(), String> {
        if bus >= crate::fx::BUSES {
            return Err("The FX bus does not exist.".into());
        }
        self.end_automation_gesture();
        self.gesture = None;
        self.return_gesture = Some(bus);
        Ok(())
    }
    /// End a point drag so the next drag or click has its own undo step.
    pub fn end_automation_gesture(&mut self) {
        let track = self.automation_gesture.take().is_some();
        if self.return_gesture.take().is_some() || track {
            self.gesture = None;
        }
    }
}
