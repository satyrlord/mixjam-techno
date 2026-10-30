//! Cover art: the cache in the data folder, and the download that fills it
//! when the user asks for a cover. The reviews folder never holds images.
use super::web_host;
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// The most bytes that one cover download may have.
pub const MAX_COVER_BYTES: u64 = 20 * 1024 * 1024;
/// The most bytes that the cache keeps. The least recently viewed covers go first.
pub const CACHE_LIMIT_BYTES: u64 = 256 * 1024 * 1024;
/// How long one download may take, from the connection to the last byte.
pub const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// The image formats that a cover can have.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageKind {
    Jpeg,
    Png,
    Webp,
}

impl ImageKind {
    const ALL: [Self; 3] = [Self::Jpeg, Self::Png, Self::Webp];

    /// The format of an image from its first bytes, or `None` for any other file.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else {
            None
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
}

/// The cover cache folder: one `<id>.<ext>` file per review.
#[derive(Clone, Debug)]
pub struct CoverCache {
    folder: PathBuf,
    limit: u64,
}

impl CoverCache {
    /// The cache in the application's data folder.
    pub fn in_data_folder() -> Self {
        Self::new(
            crate::project_io::data_root()
                .join("album-review")
                .join("covers"),
            CACHE_LIMIT_BYTES,
        )
    }

    pub fn new(folder: PathBuf, limit: u64) -> Self {
        Self { folder, limit }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The cached cover of a review, if there is one.
    pub fn path(&self, id: &str) -> Option<PathBuf> {
        ImageKind::ALL
            .into_iter()
            .map(|kind| self.folder.join(format!("{id}.{}", kind.extension())))
            .find(|path| path.is_file())
    }

    /// The bytes of a cached cover, marked as viewed now. `None` when the
    /// cache has no cover for the review.
    pub fn read(&self, id: &str) -> Option<Result<Vec<u8>, String>> {
        let path = self.path(id)?;
        let bytes =
            fs::read(&path).map_err(|error| format!("Cannot read the cached cover: {error}"));
        touch(&path);
        Some(bytes)
    }

    /// Keep a downloaded cover, replace an earlier one of the review, and
    /// remove the least recently viewed covers above the limit.
    pub fn store(&self, id: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        if !super::valid_id(id) {
            return Err(format!("\"{id}\" is not a review id."));
        }
        let kind = ImageKind::sniff(bytes).ok_or("The file is not a JPEG, PNG, or WebP image.")?;
        if bytes.len() as u64 > MAX_COVER_BYTES.min(self.limit) {
            return Err("The image is larger than the cover limit.".into());
        }
        fs::create_dir_all(&self.folder)
            .map_err(|error| format!("Cannot create the cover cache: {error}"))?;
        let path = self.folder.join(format!("{id}.{}", kind.extension()));
        crate::atomic_file::write(&path, bytes)?;
        for other in ImageKind::ALL.into_iter().filter(|other| *other != kind) {
            let _ = fs::remove_file(self.folder.join(format!("{id}.{}", other.extension())));
        }
        touch(&path);
        self.evict(&path)?;
        Ok(path)
    }

    /// The covers in the cache, oldest view first, with their sizes.
    fn entries(&self) -> Vec<(PathBuf, u64, SystemTime)> {
        let Ok(listing) = fs::read_dir(&self.folder) else {
            return Vec::new();
        };
        let mut entries: Vec<_> = listing
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let cover = path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        ImageKind::ALL
                            .iter()
                            .any(|kind| kind.extension() == extension)
                    });
                let metadata = entry.metadata().ok()?;
                (cover && metadata.is_file()).then(|| {
                    let viewed = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    (path, metadata.len(), viewed)
                })
            })
            .collect();
        entries.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
        entries
    }

    /// Remove the least recently viewed covers until the cache fits its limit.
    /// The cover at `keep` stays.
    fn evict(&self, keep: &Path) -> Result<(), String> {
        let entries = self.entries();
        let mut total: u64 = entries.iter().map(|entry| entry.1).sum();
        for (path, size, _) in entries {
            if total <= self.limit {
                break;
            }
            if path != keep {
                fs::remove_file(&path)
                    .map_err(|error| format!("Cannot trim the cover cache: {error}"))?;
                total -= size;
            }
        }
        Ok(())
    }

    /// The number of cached covers and their total size in bytes.
    pub fn usage(&self) -> (usize, u64) {
        let entries = self.entries();
        (entries.len(), entries.iter().map(|entry| entry.1).sum())
    }

    /// Remove every cached cover. Returns how many were removed.
    pub fn clear(&self) -> Result<usize, String> {
        let entries = self.entries();
        for (path, _, _) in &entries {
            fs::remove_file(path)
                .map_err(|error| format!("Cannot clear the cover cache: {error}"))?;
        }
        Ok(entries.len())
    }
}

/// Mark a cover as viewed now. A cache that cannot record it still works.
fn touch(path: &Path) {
    if let Ok(file) = fs::File::options().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

/// Whether an address can be downloaded: http or https with a host.
pub fn downloadable(address: &str) -> Result<(), String> {
    web_host(address)
        .map(|_| ())
        .ok_or_else(|| "The cover link is not an http or https address.".into())
}

/// Download a cover: http or https only, within the timeout and the size
/// limit, and only a JPEG, PNG, or WebP image.
#[cfg(feature = "gui")]
pub fn download(address: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    downloadable(address)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(user_agent())
        .build()
        .into();
    let mut response = agent
        .get(address)
        .header("Accept", "image/jpeg, image/png, image/webp")
        .call()
        .map_err(|error| match error {
            ureq::Error::StatusCode(code) => format!("The server answered HTTP {code}."),
            ureq::Error::Timeout(_) => format!(
                "The download took longer than {} seconds.",
                TIMEOUT.as_secs()
            ),
            other => format!("The download failed: {other}"),
        })?;
    if response
        .body()
        .content_length()
        .is_some_and(|length| length > MAX_COVER_BYTES)
    {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_COVER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("The download failed: {error}"))?;
    if bytes.len() as u64 > MAX_COVER_BYTES {
        return Err(too_large());
    }
    ImageKind::sniff(&bytes).ok_or("The link does not give a JPEG, PNG, or WebP image.")?;
    Ok(bytes)
}

#[cfg(feature = "gui")]
fn too_large() -> String {
    format!(
        "The image is larger than {} MB.",
        MAX_COVER_BYTES / 1024 / 1024
    )
}

/// The user agent of cover downloads, as image hosts such as Wikimedia require.
pub fn user_agent() -> String {
    format!(
        "MixJamTechno/{} (Album Review cover art; https://github.com/satyrlord/mixjam-techno)",
        crate::APP_VERSION
    )
}

/// A decoded cover, at most `MAX_SIDE` pixels on its longer side.
#[cfg(feature = "gui")]
pub struct CoverImage {
    pub width: u32,
    pub height: u32,
    /// Unpremultiplied RGBA, one row after another.
    pub rgba: Vec<u8>,
}

#[cfg(feature = "gui")]
pub const MAX_SIDE: u32 = 512;

#[cfg(feature = "gui")]
fn decode(bytes: &[u8]) -> Result<CoverImage, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| format!("The image cannot be read: {error}"))?;
    let image = if image.width() > MAX_SIDE || image.height() > MAX_SIDE {
        image.thumbnail(MAX_SIDE, MAX_SIDE)
    } else {
        image
    };
    let rgba = image.into_rgba8();
    Ok(CoverImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

/// What a cover request gave.
#[cfg(feature = "gui")]
pub enum CoverOutcome {
    Ready(CoverImage),
    /// The cache has no cover and the request did not ask to download one.
    NotCached,
    /// The reason that the cover is not available.
    Failed(String),
}

#[cfg(feature = "gui")]
pub struct CoverResult {
    pub id: String,
    pub outcome: CoverOutcome,
}

#[cfg(feature = "gui")]
struct Job {
    id: String,
    /// The address to download when the cache has no cover.
    download: Option<String>,
}

/// Loads covers on a background thread: from the cache, or by download when
/// the user asks for a cover. The window polls the results.
#[cfg(feature = "gui")]
pub struct CoverLoader {
    cache: CoverCache,
    jobs: Option<std::sync::mpsc::Sender<Job>>,
    results: Option<std::sync::mpsc::Receiver<CoverResult>>,
    /// For each review with running requests: how many, and whether one
    /// of them downloads.
    pending: std::collections::BTreeMap<String, (usize, bool)>,
}

#[cfg(feature = "gui")]
impl CoverLoader {
    pub fn new(cache: CoverCache) -> Self {
        Self {
            cache,
            jobs: None,
            results: None,
            pending: Default::default(),
        }
    }

    pub fn cache(&self) -> &CoverCache {
        &self.cache
    }

    /// Whether a request for the review is still running.
    pub fn is_pending(&self, id: &str) -> bool {
        self.pending.contains_key(id)
    }

    /// Whether a download of the review's cover is still running.
    pub fn is_downloading(&self, id: &str) -> bool {
        self.pending.get(id).is_some_and(|(_, download)| *download)
    }

    pub fn busy(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Ask for a review's cover: from the cache, or with `download`, from
    /// that address when the cache has none. A download request still runs
    /// while a cache lookup of the same review is pending.
    pub fn request(&mut self, id: &str, download: Option<&str>) {
        if self
            .pending
            .get(id)
            .is_some_and(|(_, downloading)| *downloading || download.is_none())
        {
            return;
        }
        if self.jobs.is_none() {
            let (jobs, inbox) = std::sync::mpsc::channel::<Job>();
            let (outbox, results) = std::sync::mpsc::channel();
            let cache = self.cache.clone();
            let started = std::thread::Builder::new()
                .name("album-review-covers".into())
                .spawn(move || {
                    for job in inbox {
                        let outcome = load(&cache, &job);
                        if outbox
                            .send(CoverResult {
                                id: job.id,
                                outcome,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            if started.is_err() {
                return;
            }
            self.jobs = Some(jobs);
            self.results = Some(results);
        }
        let job = Job {
            id: id.to_owned(),
            download: download.map(str::to_owned),
        };
        if self
            .jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send(job).is_ok())
        {
            let entry = self.pending.entry(id.to_owned()).or_default();
            entry.0 += 1;
            entry.1 |= download.is_some();
        }
    }

    /// The results that arrived since the last poll.
    pub fn poll(&mut self) -> Vec<CoverResult> {
        let results: Vec<CoverResult> = self
            .results
            .as_ref()
            .map(|results| results.try_iter().collect())
            .unwrap_or_default();
        for result in &results {
            if let Some(entry) = self.pending.get_mut(&result.id) {
                entry.0 -= 1;
                if entry.0 == 0 {
                    self.pending.remove(&result.id);
                }
            }
        }
        results
    }
}

#[cfg(feature = "gui")]
fn load(cache: &CoverCache, job: &Job) -> CoverOutcome {
    load_with(cache, job, download)
}

#[cfg(feature = "gui")]
fn load_with(
    cache: &CoverCache,
    job: &Job,
    download: impl FnOnce(&str) -> Result<Vec<u8>, String>,
) -> CoverOutcome {
    if let Some(cached) = cache.read(&job.id) {
        match cached.and_then(|bytes| decode(&bytes)) {
            Ok(image) => return CoverOutcome::Ready(image),
            Err(error) if job.download.is_none() => return CoverOutcome::Failed(error),
            Err(_) => {}
        }
    }
    let Some(address) = &job.download else {
        return CoverOutcome::NotCached;
    };
    match download(address).and_then(|bytes| {
        let image = decode(&bytes)?;
        cache.store(&job.id, &bytes)?;
        Ok(image)
    }) {
        Ok(image) => CoverOutcome::Ready(image),
        Err(error) => CoverOutcome::Failed(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("mixjam-covers-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    fn jpeg(size: usize) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        bytes.resize(size, 0);
        bytes
    }

    fn set_viewed(path: &Path, seconds: u64) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .unwrap();
    }

    #[test]
    fn only_jpeg_png_and_webp_images_are_covers() {
        assert_eq!(ImageKind::sniff(&jpeg(16)), Some(ImageKind::Jpeg));
        assert_eq!(
            ImageKind::sniff(b"\x89PNG\r\n\x1a\nrest"),
            Some(ImageKind::Png)
        );
        assert_eq!(
            ImageKind::sniff(b"RIFF\0\0\0\0WEBPVP8 "),
            Some(ImageKind::Webp)
        );
        for other in [&b"GIF89a..."[..], b"<html>", b"RIFF\0\0\0\0WAVEfmt ", b""] {
            assert_eq!(ImageKind::sniff(other), None);
        }
        let cache = CoverCache::new(folder("formats"), CACHE_LIMIT_BYTES);
        assert!(cache.store("album", b"<html>not an image</html>").is_err());
        assert!(cache.store("../escape", &jpeg(16)).is_err());
        assert_eq!(cache.usage(), (0, 0));
    }

    #[test]
    fn only_http_and_https_links_download() {
        for good in [
            "https://upload.wikimedia.org/wikipedia/en/a/a0/Cover.jpg",
            "http://example.com/cover.png",
        ] {
            assert!(downloadable(good).is_ok(), "{good}");
        }
        for bad in [
            "file:///C:/cover.jpg",
            "ftp://example.com/cover.jpg",
            "covers/album.jpg",
            "https://",
            "javascript:alert(1)",
        ] {
            assert!(downloadable(bad).is_err(), "{bad}");
        }
        assert!(user_agent().starts_with("MixJamTechno/"));
        assert!(user_agent().contains(crate::APP_VERSION));
    }

    #[test]
    fn a_new_cover_replaces_the_old_one_and_reading_marks_it_viewed() {
        let folder = folder("replace");
        let cache = CoverCache::new(folder.clone(), CACHE_LIMIT_BYTES);
        assert!(cache.read("album").is_none());
        let first = cache.store("album", &jpeg(100)).unwrap();
        assert_eq!(first, folder.join("album.jpg"));
        let second = cache.store("album", b"\x89PNG\r\n\x1a\n-png").unwrap();
        assert_eq!(second, folder.join("album.png"));
        assert!(!first.exists());
        assert_eq!(cache.path("album"), Some(second.clone()));
        set_viewed(&second, 10);
        assert_eq!(
            cache.read("album").unwrap().unwrap(),
            b"\x89PNG\r\n\x1a\n-png"
        );
        let viewed = fs::metadata(&second).unwrap().modified().unwrap();
        assert!(viewed > SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(10));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn the_cache_drops_the_least_recently_viewed_covers_above_its_limit() {
        let folder = folder("limit");
        let cache = CoverCache::new(folder.clone(), 1000);
        for (index, id) in ["a", "b", "c"].into_iter().enumerate() {
            let path = cache.store(id, &jpeg(300)).unwrap();
            set_viewed(&path, 100 + index as u64);
        }
        assert_eq!(cache.usage(), (3, 900));
        // Viewing "a" makes "b" the least recently viewed cover.
        cache.read("a").unwrap().unwrap();
        cache.store("d", &jpeg(300)).unwrap();
        assert_eq!(cache.usage(), (3, 900));
        assert!(cache.path("b").is_none());
        for id in ["a", "c", "d"] {
            assert!(cache.path(id).is_some(), "{id} stays");
        }
        // A cover larger than the whole cache is refused, and nothing is lost.
        assert!(cache.store("e", &jpeg(1001)).is_err());
        assert_eq!(cache.usage(), (3, 900));
        // A cover that fills the cache alone stays, and the others go.
        cache.store("f", &jpeg(1000)).unwrap();
        assert_eq!(cache.usage(), (1, 1000));
        assert!(cache.path("f").is_some());
        assert_eq!(cache.clear().unwrap(), 1);
        assert_eq!(cache.usage(), (0, 0));
        // Other files in the folder are not covers and are not removed.
        fs::write(folder.join("notes.txt"), b"keep").unwrap();
        assert_eq!(cache.clear().unwrap(), 0);
        assert!(folder.join("notes.txt").is_file());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn the_limits_match_the_plan() {
        assert_eq!(MAX_COVER_BYTES, 20 * 1024 * 1024);
        assert_eq!(CACHE_LIMIT_BYTES, 256 * 1024 * 1024);
        assert_eq!(TIMEOUT.as_secs(), 20);
    }

    #[cfg(feature = "gui")]
    #[test]
    fn an_invalid_download_is_not_cached_and_a_bad_cache_can_be_retried() {
        let folder = folder("invalid-download");
        let cache = CoverCache::new(folder.clone(), CACHE_LIMIT_BYTES);
        let job = Job {
            id: "album".into(),
            download: Some("https://example.com/cover.png".into()),
        };
        let invalid = b"\x89PNG\r\n\x1a\n-truncated";
        assert!(matches!(
            load_with(&cache, &job, |_| Ok(invalid.to_vec())),
            CoverOutcome::Failed(_)
        ));
        assert!(cache.path("album").is_none());
        cache.store("album", invalid).unwrap();
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 40, 40, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert!(matches!(
            load_with(&cache, &job, |_| Ok(png.clone())),
            CoverOutcome::Ready(_)
        ));
        assert_eq!(cache.read("album").unwrap().unwrap(), png);
        fs::remove_dir_all(folder).unwrap();
    }

    #[cfg(feature = "gui")]
    #[test]
    fn a_cached_cover_loads_on_the_background_thread_without_a_download() {
        let folder = folder("loader");
        let cache = CoverCache::new(folder.clone(), CACHE_LIMIT_BYTES);
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(600, 300, image::Rgba([200, 40, 40, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        cache.store("album", &png).unwrap();
        let mut loader = CoverLoader::new(cache);
        loader.request("album", None);
        loader.request("missing", None);
        // An address that is not http or https fails before any connection.
        loader.request("bad-link", Some("ftp://example.com/cover.jpg"));
        assert!(loader.is_pending("album"));
        assert!(!loader.is_downloading("album"));
        assert!(loader.is_downloading("bad-link"));
        let mut results = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while results.len() < 3 && std::time::Instant::now() < deadline {
            results.extend(loader.poll());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!loader.busy());
        let outcome = |id: &str| {
            &results
                .iter()
                .find(|result| result.id == id)
                .unwrap()
                .outcome
        };
        match outcome("album") {
            CoverOutcome::Ready(image) => {
                assert_eq!((image.width, image.height), (MAX_SIDE, MAX_SIDE / 2));
                assert_eq!(image.rgba.len(), (MAX_SIDE * MAX_SIDE / 2 * 4) as usize);
            }
            _ => panic!("The cached cover must load."),
        }
        assert!(matches!(outcome("missing"), CoverOutcome::NotCached));
        assert!(
            matches!(outcome("bad-link"), CoverOutcome::Failed(reason) if reason.contains("http"))
        );
        fs::remove_dir_all(folder).unwrap();
    }
}
