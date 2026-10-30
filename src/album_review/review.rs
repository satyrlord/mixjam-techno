//! One album review: its saved form and the rules a review obeys before it
//! is saved. The JSON keys are camelCase, as in the reviews folder.
use serde::{Deserialize, Serialize};

/// Whose work a review describes.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A published album by another artist, kept as reference material.
    #[default]
    Reference,
    /// The owner's own album or song.
    Own,
}

impl Kind {
    pub const ALL: [Self; 2] = [Self::Reference, Self::Own];

    pub fn key(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Own => "own",
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.key().eq_ignore_ascii_case(text.trim()))
            .ok_or_else(|| format!("The kind \"{text}\" is not reference or own."))
    }
}

/// The energy of a track, from quietest to loudest.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Energy {
    Low,
    Mid,
    High,
    Peak,
}

impl Energy {
    pub const ALL: [Self; 4] = [Self::Low, Self::Mid, Self::High, Self::Peak];

    pub fn key(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
            Self::Peak => "peak",
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|energy| energy.key().eq_ignore_ascii_case(text.trim()))
            .ok_or_else(|| format!("The energy \"{text}\" is not low, mid, high, or peak."))
    }
}

/// A timestamped moment of a track.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    /// `M:SS` or `MM:SS` from the start of the track.
    pub timestamp: String,
    /// The section that starts here, such as Intro or Breakdown.
    pub section: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Track {
    /// The track number, from 1.
    pub num: u32,
    pub title: String,
    /// `M:SS` or `MM:SS`.
    pub duration: String,
    pub energy: Energy,
    pub tags: Vec<String>,
    /// The track's part in the album, starting `Album role: `.
    pub role: String,
    pub events: Vec<Event>,
    /// Own work: the track's audio file on this computer, a WAV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_audio: Option<String>,
    /// Own work: the song file the track was made from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song_file: Option<String>,
}

/// A deep-listening analysis of one album, or of one song of the owner's.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    /// The file name without `.json`: lowercase letters, digits, single hyphens.
    pub id: String,
    pub kind: Kind,
    pub artist: String,
    pub title: String,
    pub year: i32,
    pub label: String,
    pub producer: String,
    /// The genre as one line, such as `Electronic / Ambient`.
    pub genre: String,
    pub genre_tags: Vec<String>,
    /// `M:SS` or `MM:SS` under one hour, `H:MM:SS` from one hour.
    pub runtime: String,
    /// Present, and true, only for a soundtrack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_soundtrack: Option<bool>,
    /// The remote address of the cover image. The application downloads it
    /// only when the user asks, into its own cover cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    /// The page the cover image comes from, such as the album's article.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub youtube_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bandcamp_url: Option<String>,
    /// Own work: the album's or song's audio file on this computer, a WAV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_audio: Option<String>,
    /// Own work: the song file of a song review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song_file: Option<String>,
    pub overview: String,
    pub tracks: Vec<Track>,
}

/// Text that a new, unfinished entry carries. A saved review has none.
pub const SCAFFOLD_OVERVIEW: &str = "Initial scaffold generated from MusicBrainz metadata.";
pub const SCAFFOLD_ROLE: &str = "Initial scaffold only. Narrative role pending detailed listen.";
pub const SCAFFOLD_TAGS: [&str; 2] = ["Scaffold", "Tags Pending"];
pub const SCAFFOLD_EVENT: &str = "Detailed timestamp notes pending.";
pub const ROLE_PREFIX: &str = "Album role: ";
pub const TRACK_TAGS: std::ops::RangeInclusive<usize> = 2..=5;
pub const GENRE_TAGS: std::ops::RangeInclusive<usize> = 1..=9;
pub const TRACK_EVENTS: std::ops::RangeInclusive<usize> = 2..=14;
/// Gaps between tracks and rounding let a runtime differ a little from the track sum.
pub const RUNTIME_TOLERANCE_SECONDS: u32 = 30;
const YOUTUBE_HOSTS: [&str; 3] = ["www.youtube.com", "music.youtube.com", "youtu.be"];
const AUDIO_EXTENSIONS: [&str; 1] = ["wav"];
const SONG_EXTENSIONS: [&str; 2] = ["mixjam", "mix"];

/// Seconds of an `M:SS` or `MM:SS` clock, or `None` for any other text.
pub fn clock_seconds(text: &str) -> Option<u32> {
    let (minutes, seconds) = text.split_once(':')?;
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !(1..=2).contains(&minutes.len()) || seconds.len() != 2 || !digits(minutes) {
        return None;
    }
    if !digits(seconds) {
        return None;
    }
    let seconds: u32 = seconds.parse().ok()?;
    (seconds < 60).then_some(minutes.parse::<u32>().ok()? * 60 + seconds)
}

/// Seconds of a runtime: a clock under one hour, or `H:MM:SS` from one hour.
pub fn runtime_seconds(text: &str) -> Option<u32> {
    match text.split(':').count() {
        2 => clock_seconds(text).filter(|seconds| *seconds < 3600),
        3 => {
            let (hours, rest) = text.split_once(':')?;
            let valid_hours = !hours.is_empty()
                && !hours.starts_with('0')
                && hours.bytes().all(|byte| byte.is_ascii_digit());
            if !valid_hours || rest.len() != 5 {
                return None;
            }
            let rest = clock_seconds(rest).filter(|seconds| *seconds < 3600)?;
            hours
                .parse::<u32>()
                .ok()?
                .checked_mul(3600)?
                .checked_add(rest)
        }
        _ => None,
    }
}

/// The host of an absolute http or https address, or `None`.
pub fn web_host(address: &str) -> Option<&str> {
    if address
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return None;
    }
    let lower = address.get(..8).unwrap_or(address).to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &address[8..]
    } else if lower.starts_with("http://") {
        &address[7..]
    } else {
        return None;
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.contains('@') {
        return None;
    }
    let host = if let Some((host, port)) = authority.split_once(':') {
        if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        port.parse::<u16>().ok()?;
        host
    } else {
        authority
    };
    let valid = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    valid.then_some(host)
}

fn is_https(address: &str) -> bool {
    address
        .get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

/// Whether `id` has only lowercase letters, digits, and single inner hyphens.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn extension_issue(path: &str, field: &str, allowed: &[&str], issues: &mut Vec<String>) {
    if blank(path) {
        issues.push(format!("{field} must not be empty when present"));
        return;
    }
    let extension = std::path::Path::new(path.trim())
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    if !extension.is_some_and(|extension| allowed.contains(&extension.as_str())) {
        let allowed: Vec<_> = allowed.iter().map(|ext| format!(".{ext}")).collect();
        issues.push(format!("{field} must name a {} file", allowed.join(" or ")));
    }
}

impl Review {
    /// Every rule that this review breaks; empty when it can be saved.
    pub fn issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if !valid_id(&self.id) {
            issues.push(format!(
                "id \"{}\" must use lowercase a-z, 0-9, and single hyphens",
                self.id
            ));
        }
        for (field, value) in [
            ("artist", &self.artist),
            ("title", &self.title),
            ("label", &self.label),
            ("producer", &self.producer),
            ("runtime", &self.runtime),
            ("overview", &self.overview),
        ] {
            if blank(value) {
                issues.push(format!("{field} is required"));
            }
        }
        if !(1000..=9999).contains(&self.year) {
            issues.push(format!("year {} must have four digits", self.year));
        }
        if !GENRE_TAGS.contains(&self.genre_tags.len()) {
            issues.push(format!(
                "genreTags must contain {} to {} tags",
                GENRE_TAGS.start(),
                GENRE_TAGS.end()
            ));
        }
        if self.genre_tags.iter().any(|tag| blank(tag)) {
            issues.push("genreTags entries must be non-empty".into());
        }
        if self.is_soundtrack == Some(false) {
            issues.push("isSoundtrack must be true when present".into());
        }
        self.link_issues(&mut issues);
        if self.overview.contains(SCAFFOLD_OVERVIEW) {
            issues.push("overview still has the scaffold placeholder text".into());
        }
        if self.tracks.is_empty() {
            issues.push("tracks must contain at least one track".into());
        }
        self.runtime_issues(&mut issues);
        let mut track_numbers = std::collections::BTreeSet::new();
        for (index, track) in self.tracks.iter().enumerate() {
            if !track_numbers.insert(track.num) {
                issues.push(format!(
                    "tracks[{index}].num {} repeats an earlier track number",
                    track.num
                ));
            }
            track_issues(track, &format!("tracks[{index}]"), self.kind, &mut issues);
        }
        issues
    }

    fn link_issues(&self, issues: &mut Vec<String>) {
        for (field, value) in [
            ("coverUrl", &self.cover_url),
            ("coverSource", &self.cover_source),
        ] {
            if value.as_deref().is_some_and(|url| web_host(url).is_none()) {
                issues.push(format!("{field} must be an absolute http or https address"));
            }
        }
        if let Some(url) = &self.youtube_url
            && !(is_https(url)
                && web_host(url).is_some_and(|host| {
                    YOUTUBE_HOSTS
                        .iter()
                        .any(|allowed| host.eq_ignore_ascii_case(allowed))
                }))
        {
            issues.push(
                "youtubeUrl must be an https address on www.youtube.com, music.youtube.com, \
                 or youtu.be"
                    .into(),
            );
        }
        if let Some(url) = &self.bandcamp_url
            && !(is_https(url)
                && web_host(url).is_some_and(|host| {
                    let host = host.to_ascii_lowercase();
                    host == "bandcamp.com" || host.ends_with(".bandcamp.com")
                }))
        {
            issues.push("bandcampUrl must be an https address on bandcamp.com".into());
        }
        own_file_issues(self.kind, "", &self.local_audio, &self.song_file, issues);
    }

    fn runtime_issues(&self, issues: &mut Vec<String>) {
        let Some(runtime) = runtime_seconds(&self.runtime) else {
            issues.push(format!(
                "runtime \"{}\" must use M:SS, MM:SS, or H:MM:SS",
                self.runtime
            ));
            return;
        };
        let durations: Option<Vec<u32>> = self
            .tracks
            .iter()
            .map(|track| clock_seconds(&track.duration))
            .collect();
        if let Some(durations) = durations {
            let sum: u32 = durations.iter().sum();
            let difference = runtime.abs_diff(sum);
            if difference > RUNTIME_TOLERANCE_SECONDS {
                issues.push(format!(
                    "runtime {} differs from the track sum by {difference} s (maximum \
                     {RUNTIME_TOLERANCE_SECONDS} s)",
                    self.runtime
                ));
            }
        }
    }

    /// The total length of the tracks, in seconds, when every duration is valid.
    pub fn track_seconds(&self) -> Option<u32> {
        self.tracks
            .iter()
            .map(|track| clock_seconds(&track.duration))
            .sum()
    }

    /// The review as the reviews folder stores it: pretty JSON and a final newline.
    pub fn to_file_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("A review always serializes.");
        text.push('\n');
        text
    }
}

fn own_file_issues(
    kind: Kind,
    path: &str,
    local_audio: &Option<String>,
    song_file: &Option<String>,
    issues: &mut Vec<String>,
) {
    let field = |name: &str| {
        if path.is_empty() {
            name.to_string()
        } else {
            format!("{path}.{name}")
        }
    };
    for (name, value, allowed) in [
        ("localAudio", local_audio, &AUDIO_EXTENSIONS[..]),
        ("songFile", song_file, &SONG_EXTENSIONS[..]),
    ] {
        if let Some(value) = value {
            if kind != Kind::Own {
                issues.push(format!("{} is only for a review of kind own", field(name)));
            }
            extension_issue(value, &field(name), allowed, issues);
        }
    }
}

fn track_issues(track: &Track, path: &str, kind: Kind, issues: &mut Vec<String>) {
    if track.num == 0 {
        issues.push(format!("{path}.num must start at 1"));
    }
    if blank(&track.title) {
        issues.push(format!("{path}.title is required"));
    }
    let duration = clock_seconds(&track.duration);
    if duration.is_none() {
        issues.push(format!(
            "{path}.duration \"{}\" must use M:SS or MM:SS",
            track.duration
        ));
    }
    if !TRACK_TAGS.contains(&track.tags.len()) {
        issues.push(format!(
            "{path}.tags must contain {} to {} tags",
            TRACK_TAGS.start(),
            TRACK_TAGS.end()
        ));
    }
    if track.tags.iter().any(|tag| blank(tag)) {
        issues.push(format!("{path}.tags entries must be non-empty"));
    }
    if track
        .tags
        .iter()
        .any(|tag| SCAFFOLD_TAGS.contains(&tag.as_str()))
    {
        issues.push(format!("{path}.tags still have scaffold placeholder tags"));
    }
    if track.role.strip_prefix(ROLE_PREFIX).is_none_or(blank) {
        issues.push(format!(
            "{path}.role must start with \"{ROLE_PREFIX}\" and say more"
        ));
    }
    if track.role.contains(SCAFFOLD_ROLE) {
        issues.push(format!(
            "{path}.role still has the scaffold placeholder text"
        ));
    }
    if !TRACK_EVENTS.contains(&track.events.len()) {
        issues.push(format!(
            "{path}.events must contain {} to {} events, got {}",
            TRACK_EVENTS.start(),
            TRACK_EVENTS.end(),
            track.events.len()
        ));
    }
    let mut previous: Option<u32> = None;
    for (index, event) in track.events.iter().enumerate() {
        let at = format!("{path}.events[{index}]");
        for (field, value) in [
            ("section", &event.section),
            ("description", &event.description),
        ] {
            if blank(value) {
                issues.push(format!("{at}.{field} is required"));
            }
        }
        if event.detail.as_deref().is_some_and(blank) {
            issues.push(format!("{at}.detail must not be empty when present"));
        }
        if event.description == SCAFFOLD_EVENT {
            issues.push(format!("{at}.description is the scaffold placeholder"));
        }
        let Some(start) = clock_seconds(&event.timestamp) else {
            issues.push(format!(
                "{at}.timestamp \"{}\" must use M:SS or MM:SS",
                event.timestamp
            ));
            previous = None;
            continue;
        };
        if previous.is_some_and(|previous| start <= previous) {
            issues.push(format!(
                "{at}.timestamp {} must be after the event before it",
                event.timestamp
            ));
        }
        if duration.is_some_and(|duration| start >= duration) {
            issues.push(format!(
                "{at}.timestamp {} must be before the track end ({})",
                event.timestamp, track.duration
            ));
        }
        previous = Some(start);
    }
    own_file_issues(kind, path, &track.local_audio, &track.song_file, issues);
}

/// Read a review from its JSON text: the shape first, then every rule.
pub fn parse(text: &str) -> Result<Review, String> {
    let review: Review = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let issues = review.issues();
    if issues.is_empty() {
        Ok(review)
    } else {
        Err(issues.join("; "))
    }
}
