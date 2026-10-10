//! Product identities are stable across installation locations and catalog order.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const IMPORTED_ID_START: u32 = 1_000_000;
const PRODUCT_ID_STRIDE: u32 = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Product {
    // Aliases keep projects saved under the earlier product slugs readable.
    #[serde(alias = "\x74echno2")]
    T2,
    #[serde(alias = "\x72ave")]
    T1,
    #[serde(alias = "\x64ance1")]
    D1,
    #[serde(alias = "\x64ance2")]
    D2,
    #[serde(alias = "\x64ance3")]
    D3,
    #[serde(alias = "\x64ance4")]
    D4,
    #[serde(alias = "\x68ouse")]
    H1,
    #[serde(alias = "\x74echno3")]
    T3,
}

impl Product {
    pub const ALL: [Self; 8] = [
        Self::D1,
        Self::D2,
        Self::D3,
        Self::D4,
        Self::H1,
        Self::T1,
        Self::T2,
        Self::T3,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::T2 => "T2",
            Self::T1 => "T1",
            Self::D1 => "D1",
            Self::D2 => "D2",
            Self::D3 => "D3",
            Self::D4 => "D4",
            Self::H1 => "H1",
            Self::T3 => "T3",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::T2 => "t2",
            Self::T1 => "t1",
            Self::D1 => "d1",
            Self::D2 => "d2",
            Self::D3 => "d3",
            Self::D4 => "d4",
            Self::H1 => "h1",
            Self::T3 => "t3",
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|p| p.slug().eq_ignore_ascii_case(text))
            .ok_or_else(|| format!("Unknown product: {text}."))
    }

    fn slot(self) -> u32 {
        match self {
            Self::T2 => 0,
            Self::T1 => 1,
            Self::D1 => 2,
            Self::D2 => 3,
            Self::D3 => 4,
            Self::D4 => 5,
            Self::H1 => 6,
            Self::T3 => 7,
        }
    }

    /// T2 retains its existing IDs for bounded old-project migration.
    /// Other products occupy distinct stable regions; external WAVs are separate.
    pub fn global_sample_id(self, native_id: u32) -> Result<u32, String> {
        if native_id >= PRODUCT_ID_STRIDE {
            return Err("The native sample ID exceeds its product region.".into());
        }
        Ok(self.slot() * PRODUCT_ID_STRIDE + native_id)
    }

    pub fn sample_key(global_id: u32) -> Option<(Self, u32)> {
        if global_id >= IMPORTED_ID_START {
            return None;
        }
        Self::ALL
            .into_iter()
            .find(|p| p.slot() == global_id / PRODUCT_ID_STRIDE)
            .map(|p| (p, global_id % PRODUCT_ID_STRIDE))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryBinding {
    pub product: Product,
    pub root: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_ids_are_stable_distinct_and_separate_from_imports() {
        let mut seen = std::collections::HashSet::new();
        for product in Product::ALL {
            for id in [0, 731, 13818, 65535, 99999] {
                let global = product.global_sample_id(id).unwrap();
                assert!(seen.insert(global));
                assert_eq!(Product::sample_key(global), Some((product, id)));
            }
            assert!(product.global_sample_id(100_000).is_err());
        }
        assert_eq!(Product::T2.global_sample_id(13818).unwrap(), 13818);
        assert!(Product::sample_key(IMPORTED_ID_START).is_none());
        assert!(Product::sample_key(800_000).is_none());
    }
}
