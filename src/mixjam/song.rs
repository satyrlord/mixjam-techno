//! The application's own `.mixjam` song format, format version 24.
//!
//! It adapts the preserved version-7 document to this editor. The envelope is
//! the same: strict camelCase JSON with `formatVersion`, `appVersion`, two UTC
//! timestamps, `song`, and lanes that own their placements in visible order.
//! Native songs differ from version 7 where the editor does:
//! - Ticks are the editor's 256 per bar, not MixJam's 32.
//! - `lanes` always has 64 entries with gain, pan, and two channel switches.
//! - Version 9 separates nonnegative lane amplitude from polarity. Version 8
//!   signed lane gains migrate without changing their effective amplitude.
//! - Version 10 adds lane Mute and Solo. Earlier songs open with both off.
//! - Version 11 adds `master`, the master pedalboard with the Booster pedals.
//!   Earlier songs open with every pedal off, so they sound as they did.
//! - Version 12's `master` adds the input trim and the mastering pedals.
//!   Version 11 songs open with their Booster pedals first, in their saved
//!   order, and the mastering pedals off after them.
//! - Version 13 adds `fxBuses`, the four FX buses with their effects, and
//!   each lane's `sends` to them, as version 7 saved them. Earlier songs
//!   open with every bus empty and every send at 0.
//! - Version 14 adds clip fades, clip pitch, and lane automation: a
//!   placement may hold `fades` and `pitchCents`, and a lane `automation`.
//!   Without them a clip has no fades and keeps its pitch, and a lane has no
//!   automation.
//! - Version 15 adds the channel EQ, a lane's `eq`, and the master board's
//!   `bypass`. A lane without `eq` has its EQ off at the default settings,
//!   and a board without `bypass` is not bypassed.
//! - Version 16 renames the Booster pedals' keys in `master` and its
//!   `order` (`BOOSTER_KEYS`). Versions 11 to 15 keep their earlier keys
//!   and open with the same settings; versions 16 and later accept the new keys.
//! - Version 17 adds the lane ducker, a lane's `ducker`, and the keys of the
//!   dynamics pedals (`key`) and of the FX duckers (`duckKey`). A lane
//!   without `ducker` has its ducker off at the default settings, and a
//!   pedal or effect without its key listens to its own input.
//! - Version 18 adds the channel EQ's second peak (`peak2Hz`, `peak2Db`,
//!   `peak2Q` in a lane's `eq`), the lane compressor, a lane's
//!   `compressor`, and the lane's stereo `width`. Without them the second
//!   peak is flat, the compressor is off at the default settings, and the
//!   lane is at 100% width. A lane ducker's key leaves out the master gain
//!   from version 18, so a version 17 lane ducker's threshold opens moved
//!   by `-20 * log10(masterGain)` dB, held to its range, and ducks as it did.
//! - Version 19 adds named song sections. Earlier songs have no sections.
//! - Version 20 adds the pan lane to a lane's `automation`, under `pan`.
//!   Earlier songs have no pan automation and reject the key.
//! - Version 21 adds the Repeat lane to a lane's `automation`, under
//!   `repeat`. Earlier songs have no repeat automation and reject the key.
//! - Version 22 adds the lane drive, a lane's `drive`. Without it the drive
//!   is off at the default settings; earlier songs reject the key.
//! - Version 23 adds an FX bus's return lane, `returnAutomation` in a bus of
//!   `fxBuses`, the Repeat gate lane, `repeatGate` in a lane's `automation`,
//!   and a placement's `reverse`. Earlier songs have static returns,
//!   full-length repeats, and no reversed clips, and reject the keys.
//! - Version 24 adds the Pitch lane, `pitch` in a lane's `automation`.
//!   Earlier songs have no pitch automation and reject the key.
//! - A placement names its audio by `sampleId`. Product catalog IDs resolve
//!   through `libraries`; other IDs resolve through `samples`, which link WAV
//!   files by path. Placements also keep their trim offset and gain.
//! - Missing sources, conflict choices, import notices, and the settings that
//!   the original product saved are kept, so an imported song reopens without
//!   its original file.
mod migration;
mod model;
mod reading;
#[cfg(test)]
mod tests;
mod writing;
pub(super) use reading::parse;
pub(super) use writing::save;

pub(super) const FORMAT_VERSION: u32 = 24;
