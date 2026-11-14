//! Tests of the project model, its JSON formats, and the editor.
use super::{load::MAX_PROJECT_BYTES, *};
use std::{collections::BTreeMap, fs, path::PathBuf};

mod editor;
mod json_projects;
mod model;
mod tempo;
