//! Reading the settings that earlier versions saved: raw values kept until the
//! version is known, duplicate-key checks, and the Booster board layouts.

use crate::master::{BoosterCompressor, BoosterEq, BoosterWide, MasterChain, PedalKind};
use serde::{Deserialize, Serialize};

/// The channel EQ's keys that version 18 added.
pub(super) const PEAK2_KEYS: [&str; 3] = ["peak2Hz", "peak2Db", "peak2Q"];

/// The Booster pedals' keys in a saved board and its `order`: the key of
/// versions 11 to 15, then the key of version 16 and later. The earlier
/// keys must match the saved files byte for byte.
pub(super) const BOOSTER_KEYS: [(&str, &str); 3] = [
    ("ejayEq", "boosterEq"),
    ("ejayCompressor", "boosterCompressor"),
    ("ejayWide", "boosterWide"),
];

/// A lane ducker's threshold for a key without the master gain, from a
/// version 17 song, whose key held the master gain. The key now reads
/// `-20 * log10(masterGain)` dB hotter, so the threshold moves by as much
/// and the lane ducks as it did, as far as the threshold's range allows. A
/// master gain of 0 or less left the key silent, as it still is: the
/// threshold stays. So does a threshold outside its range, which validation
/// then rejects, as it did before.
pub(super) fn threshold_without_master_gain(threshold: f32, master_gain: f32) -> f32 {
    let range = crate::ducker::DuckerParam::Threshold.range();
    if !(master_gain.is_finite() && master_gain > 0.0)
        || !(range.low..=range.high).contains(&threshold)
    {
        return threshold;
    }
    let moved = f64::from(threshold) - 20.0 * f64::from(master_gain).log10();
    (moved as f32).clamp(range.low, range.high)
}

/// Delay settings admission until the document version is known. Retain raw
/// input so explicit zero keys and duplicate fields are not lost. Save typed
/// settings directly to preserve their f32 precision and normal formatting.
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Saved<T> {
    Known(T),
    Raw(Box<serde_json::value::RawValue>),
}

impl<'de, T> Deserialize<'de> for Saved<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Box::<serde_json::value::RawValue>::deserialize(deserializer).map(Self::Raw)
    }
}

impl<T: Serialize> Saved<T> {
    pub(super) fn json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

impl<T: Serialize + serde::de::DeserializeOwned> Saved<T> {
    /// Renaming legacy keys needs a JSON object, but ordinary Value parsing
    /// discards duplicate fields before the typed settings can reject them.
    pub(super) fn migration_json(&self) -> Result<serde_json::Value, String> {
        match self {
            Self::Known(settings) => serde_json::to_value(settings),
            Self::Raw(raw) => serde_json::from_str::<UniqueJson>(raw.get()).map(|json| json.0),
        }
        .map_err(|error| format!("The master pedalboard is invalid: {error}"))
    }

    pub(super) fn settings(self) -> Result<T, serde_json::Error> {
        match self {
            Self::Known(settings) => Ok(settings),
            Self::Raw(raw) => serde_json::from_str(raw.get()),
        }
    }
}

/// A JSON value whose objects reject duplicate decoded field names. Nested
/// settings use the same rule before an older board's keys are renamed.
pub(super) struct UniqueJson(pub(super) serde_json::Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = serde_json::Value;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("JSON without duplicate fields")
            }

            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(value.into())
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(value.into())
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(value.into())
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| E::custom("A JSON number must be finite."))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(value.into())
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(value.into())
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(serde_json::Value::Null)
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueJson(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(serde_json::Value::Array(values))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut fields: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = fields.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom(format!("duplicate field `{key}`")));
                    }
                    let UniqueJson(value) = fields.next_value()?;
                    values.insert(key, value);
                }
                Ok(serde_json::Value::Object(values))
            }
        }
        deserializer.deserialize_any(Visitor).map(Self)
    }
}

/// The version 11 `master` record: the Booster pedals alone, with the
/// keys of version 16.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Booster {
    order: [BoosterPedal; 3],
    booster_eq: BoosterEq,
    booster_compressor: BoosterCompressor,
    booster_wide: BoosterWide,
}

#[derive(Clone, Copy, Deserialize)]
enum BoosterPedal {
    #[serde(rename = "boosterEq")]
    Eq,
    #[serde(rename = "boosterCompressor")]
    Compressor,
    #[serde(rename = "boosterWide")]
    Wide,
}

impl Booster {
    /// The Booster pedals lead the board in their saved order; the mastering
    /// pedals follow, off.
    pub(super) fn into_board(self) -> MasterChain {
        let mut chain = MasterChain {
            booster_eq: self.booster_eq,
            booster_compressor: self.booster_compressor,
            booster_wide: self.booster_wide,
            ..MasterChain::default()
        };
        chain.order[..3].copy_from_slice(&self.order.map(|pedal| match pedal {
            BoosterPedal::Eq => PedalKind::BoosterEq,
            BoosterPedal::Compressor => PedalKind::BoosterCompressor,
            BoosterPedal::Wide => PedalKind::BoosterWide,
        }));
        chain
    }
}

/// A version 11 to 15 board with the Booster pedals' earlier keys, in
/// the record and in its `order`, changed to the current keys. These
/// versions reject the current keys.
pub(super) fn current_booster_keys(
    mut master: serde_json::Value,
    version: u32,
) -> Result<serde_json::Value, String> {
    let rejected = |key: &str| format!("Version {version} songs cannot contain master {key}.");
    let Some(record) = master.as_object_mut() else {
        return Ok(master);
    };
    for (earlier, current) in BOOSTER_KEYS {
        if record.contains_key(current) {
            return Err(rejected(current));
        }
        if let Some(pedal) = record.remove(earlier) {
            record.insert(current.into(), pedal);
        }
    }
    if let Some(order) = record
        .get_mut("order")
        .and_then(serde_json::Value::as_array_mut)
    {
        for name in order {
            for (earlier, current) in BOOSTER_KEYS {
                if name.as_str() == Some(current) {
                    return Err(rejected(current));
                }
                if name.as_str() == Some(earlier) {
                    *name = current.into();
                }
            }
        }
    }
    Ok(master)
}
