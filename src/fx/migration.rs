//! Read the FX settings of earlier songs and JSON projects.

use super::FxBuses;
use crate::unique_json::UniqueJson;
use serde::Deserialize;
use serde_json::{Map, Value};

const V27_KEYS: [&str; 7] = [
    "levelMatch",
    "linkTimes",
    "smearPercent",
    "filterResonance",
    "bassMult",
    "trebleMult",
    "duckThresholdDb",
];

pub(crate) fn migrate_earlier_module(
    module: &mut Map<String, Value>,
    version: u32,
) -> Result<(), String> {
    if let Some(key) = V27_KEYS.into_iter().find(|key| module.contains_key(*key)) {
        return Err(format!(
            "Version {version} songs cannot contain FX version 27 parameter `{key}`."
        ));
    }
    add_missing_defaults(module);
    Ok(())
}

fn add_missing_defaults(module: &mut Map<String, Value>) {
    match module.get("type").and_then(Value::as_str) {
        Some("echoform-delay") => {
            // The saved milliseconds represent f32 values. Equal numbers can
            // have different JSON spellings, such as 250 and 250.0.
            let time = |key| module.get(key).and_then(Value::as_f64).map(|ms| ms as f32);
            let linked = time("timeMsL") == time("timeMsR")
                && module.get("divisionL") == module.get("divisionR");
            module.entry("linkTimes").or_insert(linked.into());
            module.entry("smearPercent").or_insert(0.0.into());
            module.entry("filterResonance").or_insert(0.0.into());
        }
        Some("aetherform-reverb") => {
            module.entry("bassMult").or_insert(1.0.into());
            module.entry("trebleMult").or_insert(1.0.into());
        }
        Some("emberform-drive") => {
            module.entry("levelMatch").or_insert(true.into());
        }
        _ => return,
    }
    module.entry("duckThresholdDb").or_insert((-27.0).into());
}

pub(crate) fn module_in_bus(bus: &mut Value) -> Option<&mut Map<String, Value>> {
    match bus {
        Value::Object(record) => record.get_mut("module"),
        Value::Array(fields) => fields.first_mut(),
        _ => None,
    }
    .and_then(Value::as_object_mut)
}

/// An earlier JSON project has no version for its FX settings. Keep the new
/// fields it has and supply the migration defaults for the ones it omits.
pub(crate) fn deserialize_project_fx<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<FxBuses, D::Error> {
    let UniqueJson(mut json) = UniqueJson::deserialize(deserializer)?;
    if let Some(buses) = json.as_array_mut() {
        for module in buses.iter_mut().filter_map(module_in_bus) {
            add_missing_defaults(module);
        }
    }
    serde_json::from_value(json).map_err(serde::de::Error::custom)
}
