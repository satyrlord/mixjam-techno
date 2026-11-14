//! Starting one agent request: checking it against the window and turning it
//! into input steps or an immediate reply, and the edit a clip request makes.
use super::*;

#[path = "request/arrangement.rs"]
mod arrangement;
#[path = "request/input.rs"]
mod input;
#[path = "request/queries.rs"]
mod queries;

impl MixJamApp {
    pub(super) fn agent_start(
        &mut self,
        ctx: &egui::Context,
        request: Request,
    ) -> Result<Start, String> {
        if matches!(
            request,
            Request::Place { .. }
                | Request::MoveClips { .. }
                | Request::DeleteClips { .. }
                | Request::Select { .. }
                | Request::Seek { .. }
                | Request::SetLoop { .. }
                | Request::SetSections { .. }
                | Request::Automate { .. }
                | Request::Clip { .. }
        ) && let Some(modal) = self.blocking_modal(ctx)
        {
            return Err(format!(
                "Arrangement commands are disabled while the {modal} modal is open."
            ));
        }
        match request {
            Request::Screenshot { .. }
            | Request::Click { .. }
            | Request::Set { .. }
            | Request::Key { .. }
            | Request::Type { .. }
            | Request::Pointer { .. }
            | Request::Scroll { .. }
            | Request::Wait { .. }
            | Request::Quit => self.agent_input(ctx, request),
            Request::Place { .. }
            | Request::MoveClips { .. }
            | Request::DeleteClips { .. }
            | Request::Select { .. }
            | Request::Seek { .. }
            | Request::SetLoop { .. }
            | Request::SetSections { .. }
            | Request::Automate { .. }
            | Request::Clip { .. } => self.agent_arrangement(request),
            Request::Help
            | Request::State
            | Request::Samples { .. }
            | Request::Ui { .. }
            | Request::Libraries
            | Request::SetLibraries { .. }
            | Request::Reviews { .. }
            | Request::Review { .. }
            | Request::ReviewTracks { .. }
            | Request::AddReview { .. }
            | Request::UpdateReview { .. }
            | Request::Capture { .. } => self.agent_query(ctx, request),
        }
    }
}

/// The edit for a `clip` request: `clip` with the request's fields changed.
pub(super) fn clip_edit(clip: &Clip, request: Request) -> Result<EditCommand, String> {
    let Request::Clip {
        gain_db,
        offset_frames,
        length_ticks,
        fade_in_ticks,
        fade_in_curve,
        fade_out_ticks,
        fade_out_curve,
        pitch_cents,
        reverse,
        ..
    } = request
    else {
        return Err("The request does not change a clip.".into());
    };
    let gain = match gain_db {
        Some(db) if db.is_finite() && (-60.0..=clip_gain_db(2.0)).contains(&db) => {
            clip_gain_from_db(db)
        }
        Some(_) => return Err("gain_db must be from -60 (silence) to 6.".into()),
        None => clip.gain,
    };
    let length_ticks = length_ticks.unwrap_or(clip.length_ticks);
    if length_ticks == 0 {
        return Err("length_ticks must be at least 1.".into());
    }
    // A fade that is given must fit. A fade left out is shortened with the
    // clip, as a resize in the window does.
    let mut fades = clip.fades;
    fades.fade_in.ticks = fade_in_ticks.unwrap_or(fades.fade_in.ticks);
    fades.fade_out.ticks = fade_out_ticks.unwrap_or(fades.fade_out.ticks);
    fades.fade_in.curve = fade_in_curve.unwrap_or(fades.fade_in.curve);
    fades.fade_out.curve = fade_out_curve.unwrap_or(fades.fade_out.curve);
    if (fade_in_ticks.is_some() || fade_out_ticks.is_some()) && fades.fit(length_ticks) != fades {
        return Err("The fades together are longer than the clip.".into());
    }
    Ok(EditCommand::SetClip {
        id: clip.id,
        start_tick: clip.start_tick,
        gain,
        offset_frames: offset_frames.unwrap_or(clip.offset_frames),
        length_ticks,
        fades,
        pitch_cents: pitch_cents.unwrap_or(clip.pitch_cents),
        reverse: reverse.unwrap_or(clip.reverse),
    })
}
