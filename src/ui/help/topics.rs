//! The pages of the manual, and what each one has: its name, its window,
//! its palette, and its content.
use super::content::{
    ALBUM_REVIEW, ARRANGE, AUTOMATION, Block, EFFECTS, FX_RETURNS, MASTERING, MIX_VIEW, MIXER,
    RECORDING, SAMPLE_EDITOR, SHORTCUTS, STEP_SYNTH, STRETCH, WORKSPACE,
};
use super::*;

/// A page of Help.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(in super::super) enum Topic {
    #[default]
    Shortcuts,
    Arrange,
    Automation,
    Recording,
    Mixer,
    MixView,
    Mastering,
    FxReturns,
    Workspace,
    Stretch,
    Effects,
    StepSynth,
    SampleEditor,
    AlbumReview,
}

impl Topic {
    pub(in super::super) const ALL: [Topic; 14] = [
        Topic::Shortcuts,
        Topic::Arrange,
        Topic::Automation,
        Topic::Recording,
        Topic::Mixer,
        Topic::MixView,
        Topic::Mastering,
        Topic::FxReturns,
        Topic::Workspace,
        Topic::Stretch,
        Topic::Effects,
        Topic::StepSynth,
        Topic::SampleEditor,
        Topic::AlbumReview,
    ];

    pub(in super::super) fn name(self) -> &'static str {
        match self {
            Topic::Shortcuts => "Shortcuts",
            Topic::Arrange => "Arrange",
            Topic::Automation => "Automation",
            Topic::Recording => "Recording",
            Topic::Mixer => "Mixer",
            Topic::MixView => "Mix view",
            Topic::Mastering => "Mastering",
            Topic::FxReturns => "FX returns",
            Topic::Workspace => "Workspace",
            Topic::Stretch => "Time Stretcher",
            Topic::Effects => "Effect Studio",
            Topic::StepSynth => "Step synth",
            Topic::SampleEditor => "Sample editor",
            Topic::AlbumReview => "Album Review",
        }
    }

    /// The page's own window, which the `?` of its tool or board opens.
    pub(in super::super) fn window(self) -> Option<&'static str> {
        match self {
            Topic::Stretch => Some("Time Stretcher help"),
            Topic::Effects => Some("Effect Studio help"),
            Topic::StepSynth => Some("Step synth help"),
            Topic::SampleEditor => Some("Sample editor help"),
            Topic::Mastering => Some("Mastering help"),
            _ => None,
        }
    }

    /// The tool palette of the page, which its window and its mark take.
    pub(in super::super) fn palette(self) -> Option<controls::Palette> {
        match self {
            Topic::Stretch | Topic::FxReturns => Some(controls::Palette::Marine),
            Topic::Effects => Some(controls::Palette::Red),
            Topic::StepSynth => Some(controls::Palette::Olive),
            Topic::SampleEditor => Some(controls::Palette::Granite),
            Topic::Mastering => Some(controls::Palette::Indigo),
            _ => None,
        }
    }

    /// The floating tool window that the page's window opens beside. Board
    /// pages open beside the lower panel instead.
    pub(super) fn tool_window(self) -> Option<&'static str> {
        match self {
            Topic::Stretch => Some("Time Stretcher"),
            Topic::Effects => Some("Effect Studio"),
            Topic::SampleEditor => Some("Sample editor"),
            _ => None,
        }
    }

    pub(super) fn blocks(self) -> &'static [Block] {
        match self {
            Topic::Shortcuts => SHORTCUTS,
            Topic::Arrange => ARRANGE,
            Topic::Automation => AUTOMATION,
            Topic::Recording => RECORDING,
            Topic::Mixer => MIXER,
            Topic::MixView => MIX_VIEW,
            Topic::Mastering => MASTERING,
            Topic::FxReturns => FX_RETURNS,
            Topic::Workspace => WORKSPACE,
            Topic::Stretch => STRETCH,
            Topic::Effects => EFFECTS,
            Topic::StepSynth => STEP_SYNTH,
            Topic::SampleEditor => SAMPLE_EDITOR,
            Topic::AlbumReview => ALBUM_REVIEW,
        }
    }

    /// Every line of the page, in order. Only the library's UI tests use it.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(in super::super) fn text(self) -> Vec<&'static str> {
        let mut lines = Vec::new();
        for block in self.blocks() {
            match block {
                Block::Heading(text) => lines.push(*text),
                Block::Facts(facts) | Block::Steps(facts) => lines.extend(facts.iter()),
                Block::Keys(rows) => {
                    for (action, keys, note) in *rows {
                        lines.push(*action);
                        lines.extend(keys.iter());
                        lines.push(*note);
                    }
                }
                Block::Terms(rows) => {
                    for (term, meaning) in *rows {
                        lines.push(*term);
                        lines.push(*meaning);
                    }
                }
            }
        }
        lines.retain(|line| !line.is_empty());
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::mixer::{GAIN_FINE_STEP_DB, GAIN_STEP_DB, useful_taper_span};
    use super::*;
    use mixjam_techno::application::{
        INPUT_PEAK_LAMP_DBFS, MAX_MIXER_GAIN_DB, OVER_DBTP, VU_REFERENCE_DBFS,
        fx::return_limiter_ceiling_dbfs,
    };

    fn page_says(topic: Topic, fact: &str) -> bool {
        topic.text().iter().any(|line| line.contains(fact))
    }

    #[test]
    fn the_manual_quotes_the_gain_and_peak_limits_the_application_defines() {
        let top = format!("{MAX_MIXER_GAIN_DB:+.0} dB");
        assert!(
            page_says(Topic::Mixer, &format!("to {top};")),
            "the mixer page must quote the top of the gain range, {top}"
        );
        let ceiling = format!("{OVER_DBTP:.0} dBTP");
        assert!(
            page_says(Topic::Mastering, &format!("ceiling at {ceiling}")),
            "the mastering page must quote the limiter ceiling, {ceiling}"
        );
        assert!(
            page_says(Topic::Mastering, &format!("{OVER_DBTP:.0} dBFS")),
            "the mastering page must quote the peak lamp level, {OVER_DBTP:.0} dBFS"
        );
    }

    #[test]
    fn the_manual_names_both_clip_copy_modifiers_that_a_drop_accepts() {
        // `drag.rs` copies a dropped clip while Shift or Alt is held.
        assert!(
            page_says(Topic::Arrange, "Shift-drag or Alt-drag copies it"),
            "the arrange page must name Shift and Alt as copy modifiers"
        );
        assert!(
            page_says(Topic::Shortcuts, "Shift or Alt copies it"),
            "the shortcut sheet must name Shift and Alt as copy modifiers"
        );
    }

    #[test]
    fn the_manual_quotes_the_mixer_gain_step_and_taper_range() {
        let step = format!("{GAIN_STEP_DB:.1} dB");
        assert!(
            page_says(Topic::Mixer, &step),
            "the mixer page must quote the {step} arrow gain step"
        );
        let fine = format!("{GAIN_FINE_STEP_DB:.1} dB");
        assert!(
            page_says(Topic::Mixer, &fine),
            "the mixer page must quote the {fine} fine gain step"
        );
        let (percent, low, high) = useful_taper_span();
        let travel = format!("{percent:.0}% of its travel to {low:.0} through {high:+.0} dB");
        assert!(
            page_says(Topic::Mixer, &travel),
            "the mixer page must quote the taper travel: {travel}"
        );
        let top = format!("{MAX_MIXER_GAIN_DB:+.0} dB");
        assert!(
            page_says(Topic::Mixer, &top),
            "the mixer page must quote the +End key gain limit {top}"
        );
    }

    #[test]
    fn the_manual_quotes_mastering_and_fx_meter_and_limiter_references() {
        let vu_ref = format!("0 VU is {VU_REFERENCE_DBFS:.0} dBFS");
        assert!(
            page_says(Topic::Mastering, &vu_ref),
            "the mastering page must quote the VU reference: {vu_ref}"
        );
        let peak_lamp = format!("light at {INPUT_PEAK_LAMP_DBFS:.0} dBFS");
        assert!(
            page_says(Topic::Mastering, &peak_lamp),
            "the mastering page must quote the input peak lamp threshold: {peak_lamp}"
        );
        let over_tp = format!("passes {OVER_DBTP:.0} dBTP");
        assert!(
            page_says(Topic::Mastering, &over_tp),
            "the mastering page must quote the OVER true-peak threshold: {over_tp}"
        );
        let ceiling = format!("{:.6}", return_limiter_ceiling_dbfs());
        let ceiling = ceiling.trim_end_matches('0').trim_end_matches('.');
        let fx_ceiling = format!("below {ceiling} dBFS");
        assert!(
            page_says(Topic::FxReturns, &fx_ceiling),
            "the FX returns page must quote the bus limiter ceiling: {fx_ceiling}"
        );
    }
}
