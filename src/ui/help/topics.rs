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
    use super::*;
    use mixjam_techno::application::{MAX_MIXER_GAIN_DB, OVER_DBTP};

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
}
