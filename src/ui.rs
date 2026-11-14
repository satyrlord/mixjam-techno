#[path = "ui/capture.rs"]
mod capture;
use capture::write_bmp;
#[path = "ui/view.rs"]
mod view;
use view::*;
#[path = "ui/actions.rs"]
mod actions;
use actions::*;
#[path = "ui/agent.rs"]
mod agent;
#[path = "ui/album_review.rs"]
mod album_review;
#[path = "ui/app.rs"]
mod app;
pub use app::MixJamApp;
use app::*;
#[path = "ui/commands.rs"]
mod commands;
#[path = "ui/controls.rs"]
mod controls;
#[path = "ui/dialog.rs"]
mod dialog;
#[path = "ui/drag.rs"]
mod drag;
use drag::*;
#[path = "ui/effects.rs"]
mod effects;
#[path = "ui/eq.rs"]
mod eq;
#[path = "ui/fx.rs"]
mod fx;
#[path = "ui/input.rs"]
mod input;
#[path = "ui/inspector.rs"]
mod inspector;
#[path = "ui/lanes.rs"]
mod lanes;
#[path = "ui/libraries.rs"]
mod libraries;
#[path = "ui/master.rs"]
mod master;
#[path = "ui/mix_view.rs"]
mod mix_view;
#[path = "ui/mixer.rs"]
mod mixer;
#[path = "ui/modals.rs"]
mod modals;
#[path = "ui/options.rs"]
mod options;
#[path = "ui/overview.rs"]
mod overview;
#[path = "ui/palette.rs"]
mod palette;
use palette::*;
#[path = "ui/paths.rs"]
mod paths;
use paths::*;
#[path = "ui/room.rs"]
mod room;
#[path = "ui/sample_editor.rs"]
mod sample_editor;
#[path = "ui/selection.rs"]
mod selection;
#[path = "ui/sounds.rs"]
mod sounds;
#[path = "ui/step_synth.rs"]
mod step_synth;
#[path = "ui/stretch.rs"]
mod stretch;
#[path = "ui/theme.rs"]
mod theme;
#[path = "ui/tools.rs"]
mod tools;
use tools::*;
#[path = "ui/waveform.rs"]
mod waveform;
use theme::roles;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};

use crate::ui_icons::{RailIcon, RailIcons};
use mixjam_techno::application::{
    Clip, EQ_BAND_MAX, EQ_FREQUENCIES, EditCommand, EffectsState, LibraryView, MAX_LIBRARY_FOLDERS,
    MasterChain, MeterReading, MissingReference, MissingSample, Pcm, PedalKind, Product, Project,
    SUPPORTED_BPMS, Sample, Session, StepSynthState, StretchSettings, TICKS_PER_BAR, TRACK_COUNT,
    TrackSwitch, automation,
};
