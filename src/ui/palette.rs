//! The Sounds panel's palette: its sort order, the filtered sample indices
//! and category names cached per catalog, and a sound's product.
use super::*;

/// What the filtered palette depends on.
#[derive(PartialEq)]
pub(super) struct PaletteKey {
    pub(super) catalog_revision: u64,
    /// The catalog's address and size also tell one session from another.
    pub(super) catalog: (usize, usize),
    imported: usize,
    pub(super) search: String,
    pub(super) group: Option<String>,
    pub(super) product: Option<Product>,
    pub(super) sort: PaletteSort,
    descending: bool,
}

/// Palette order. Ties keep library order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum PaletteSort {
    #[default]
    Library,
    Name,
    Length,
    Bpm,
    Category,
}

impl PaletteSort {
    pub(super) const ALL: [Self; 5] = [
        Self::Library,
        Self::Name,
        Self::Length,
        Self::Bpm,
        Self::Category,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Library => "Library order",
            Self::Name => "By name",
            Self::Length => "By length",
            Self::Bpm => "By BPM",
            Self::Category => "By category",
        }
    }

    /// The order this sort gives, then the order it gives reversed.
    pub(super) fn directions(self) -> [&'static str; 2] {
        match self {
            Self::Library => ["Library order", "Reversed library order"],
            Self::Name | Self::Category => ["A to Z", "Z to A"],
            Self::Length => ["Shortest first", "Longest first"],
            Self::Bpm => ["Slowest first", "Fastest first"],
        }
    }
}

/// The filtered sample indices and the category names of one catalog.
pub(super) struct PaletteCache {
    pub(super) key: PaletteKey,
    pub(super) indices: Rc<Vec<usize>>,
    pub(super) groups: Rc<Vec<String>>,
}

impl MixJamApp {
    pub(super) fn sample_product(&self, id: u32) -> Option<Product> {
        Product::sample_key(id)
            .map(|(product, _)| product)
            .or_else(|| {
                self.session
                    .project()
                    .imported_samples
                    .iter()
                    .find(|sample| sample.id == id)
                    .and_then(|sample| sample.origin)
            })
    }

    /// The filtered palette and its category names. Both scan the whole
    /// catalog, so they are kept until a filter or the catalog changes.
    pub(super) fn palette_view(&mut self) -> (Rc<Vec<usize>>, Rc<Vec<String>>) {
        let key = PaletteKey {
            catalog_revision: self.session.catalog_revision(),
            catalog: {
                let samples = self.session.library().samples;
                (samples.as_ptr() as usize, samples.len())
            },
            imported: self.session.project().imported_samples.len(),
            search: self.sounds.search.clone(),
            group: self.sounds.group.clone(),
            product: self.sounds.product,
            sort: self.sounds.sort,
            descending: self.sounds.descending,
        };
        if let Some(cache) = &self.sounds.cache
            && cache.key == key
        {
            return (cache.indices.clone(), cache.groups.clone());
        }
        let indices = Rc::new(self.palette_indices());
        let groups: BTreeMap<String, &str> = self
            .session
            .library()
            .samples
            .iter()
            .map(|sample| sample.group_name.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|name| (name.to_lowercase(), name))
            .collect();
        let groups = Rc::new(groups.into_values().map(str::to_owned).collect());
        self.sounds.cache = Some(PaletteCache {
            key,
            indices: Rc::clone(&indices),
            groups: Rc::clone(&groups),
        });
        (indices, groups)
    }

    pub(super) fn palette_indices(&self) -> Vec<usize> {
        let query = self.sounds.search.to_lowercase();
        // One lookup table for the whole palette, not one search per sample.
        let origins: std::collections::HashMap<u32, Product> = if self.sounds.product.is_some() {
            self.session
                .project()
                .imported_samples
                .iter()
                .filter_map(|sample| Some((sample.id, sample.origin?)))
                .collect()
        } else {
            Default::default()
        };
        let samples = self.session.library().samples;
        let mut indices: Vec<usize> = samples
            .iter()
            .enumerate()
            .filter_map(|(index, sample)| {
                (self
                    .sounds
                    .group
                    .as_deref()
                    .is_none_or(|group| sample.group_name.eq_ignore_ascii_case(group))
                    && self.sounds.product.is_none_or(|product| {
                        Product::sample_key(sample.id)
                            .map(|(product, _)| product)
                            .or_else(|| origins.get(&sample.id).copied())
                            == Some(product)
                    })
                    && (query.is_empty() || sample.name.to_lowercase().contains(&query)))
                .then_some(index)
            })
            .collect();
        // Stable sorts: equal keys keep library order in either direction.
        let descending = self.sounds.descending;
        let direct = |order: std::cmp::Ordering| {
            if descending { order.reverse() } else { order }
        };
        let text = |text: &str| {
            let key = text.to_lowercase();
            if descending {
                Err(std::cmp::Reverse(key))
            } else {
                Ok(key)
            }
        };
        match self.sounds.sort {
            PaletteSort::Library if descending => indices.reverse(),
            PaletteSort::Library => {}
            PaletteSort::Name => indices.sort_by_cached_key(|&index| text(&samples[index].name)),
            PaletteSort::Length => {
                // Bars first, as the tiles show them; audio duration orders equal bar counts.
                indices.sort_by(|&a, &b| {
                    let key = |sample: &Sample| {
                        let seconds =
                            f64::from(sample.frame_count) / f64::from(sample.sample_rate.max(1));
                        (sample.bars, seconds)
                    };
                    let (a, b) = (key(&samples[a]), key(&samples[b]));
                    direct(a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)))
                });
            }
            PaletteSort::Bpm => indices
                .sort_by(|&a, &b| direct(samples[a].source_bpm.total_cmp(&samples[b].source_bpm))),
            PaletteSort::Category => {
                indices.sort_by_cached_key(|&index| text(&samples[index].group_name));
            }
        }
        indices
    }
}
