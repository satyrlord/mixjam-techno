//! Scope row and response-curve widgets for the 3-theme Channel Strip window
//! and the Track Inspector.
use super::super::*;
use mixjam_techno::application::{
    TrackMeterReading,
    drive::{Drive, sample_drive_curve},
    eq::{ChannelEq, sample_channel_eq_curve},
    options::MeterDisplayRate,
};
use std::time::Duration;

const CURVE_POINTS: usize = 160;
const DRIVE_POINTS: usize = 128;
const GR_HISTORY_LEN: usize = 72;
const EQ_MAX_DB: f32 = 18.0;
const GR_MAX_DB: f32 = 24.0;

#[derive(Clone, Copy)]
pub(super) struct Dynamics {
    reading: Option<(u32, TrackMeterReading)>,
    interval: Duration,
}

impl Dynamics {
    pub(super) fn read(session: &Session, track: usize) -> Self {
        Self {
            reading: session
                .playing_mix()
                .then(|| (session.master_meters().run, session.track_meters(track))),
            interval: Duration::from_millis(
                if session.options().meter_display_rate == MeterDisplayRate::Reduced {
                    67
                } else {
                    33
                },
            ),
        }
    }

    pub(super) fn show(self, ui: &egui::Ui, rect: Rect, track: usize) {
        let now = ui.input(|input| input.time);
        let id = ui.id().with("track-reduction-history");
        let traces = ui.ctx().data_mut(|data| {
            let history = data.get_temp_mut_or_default::<GrHistory>(id);
            if let Some((run, reading)) = self.reading {
                history.record(run, reading, now, self.interval);
            } else {
                *history = GrHistory::default();
            }
            history.samples
        });
        if self.reading.is_some() {
            ui.ctx().request_repaint_after(self.interval);
        }

        let gap = 4.0;
        let width = (rect.width() - gap) * 0.5;
        let size = Vec2::new(width, rect.height());
        for (index, (title, processor, accent)) in [
            ("Comp GR", "compressor", roles().sections[4]),
            ("Duck GR", "ducker", roles().sections[2]),
        ]
        .into_iter()
        .enumerate()
        {
            let min = rect.min + Vec2::new(index as f32 * (width + gap), 0.0);
            let history = &traces[index];
            let reduction = history[GR_HISTORY_LEN - 1];
            controls::instrument::gr_history_scope(
                ui,
                Rect::from_min_size(min, size),
                title,
                history,
                GR_MAX_DB,
                reduction,
                accent,
            );
            ui.ctx()
                .accesskit_node_builder(ui.id().with(title), |node| {
                    node.set_role(egui::accesskit::Role::Meter);
                    node.set_label(format!("Track {} {processor} gain reduction", track + 1));
                    node.set_numeric_value(f64::from(reduction));
                });
        }
    }
}

#[derive(Clone)]
struct GrHistory {
    samples: [[f32; GR_HISTORY_LEN]; 2],
    pending: [f32; 2],
    run: Option<u32>,
    next: f64,
    /// The time of the last reading. A window that comes back after a gap,
    /// such as a return to its track, starts a new history.
    seen: Option<f64>,
}

/// A gap in readings longer than this starts a new history.
const GR_STALE_SECONDS: f64 = 0.5;

impl Default for GrHistory {
    fn default() -> Self {
        Self {
            samples: [[0.0; GR_HISTORY_LEN]; 2],
            pending: [0.0; 2],
            run: None,
            next: 0.0,
            seen: None,
        }
    }
}

impl GrHistory {
    fn record(&mut self, run: u32, reading: TrackMeterReading, now: f64, interval: Duration) {
        let stale = self.seen.is_some_and(|seen| now - seen > GR_STALE_SECONDS);
        if self.run != Some(run) || stale {
            *self = Self {
                run: Some(run),
                ..Self::default()
            };
        }
        self.seen = Some(now);
        self.pending[0] = self.pending[0].max(reading.compressor_gr_db);
        self.pending[1] = self.pending[1].max(reading.ducker_gr_db);
        if now >= self.next {
            for (samples, peak) in self.samples.iter_mut().zip(self.pending) {
                samples.copy_within(1.., 0);
                samples[GR_HISTORY_LEN - 1] = peak;
            }
            self.pending = [0.0; 2];
            self.next = now + interval.as_secs_f64();
        }
    }
}

/// Draw the track's frequency-response curve inside `rect` and register its
/// AccessKit region as `"{name} response curve"`. Shared with the Inspector.
pub(in super::super) fn response_curve(ui: &mut egui::Ui, rect: Rect, eq: &ChannelEq, name: &str) {
    let curve = sample_channel_eq_curve(eq, CURVE_POINTS);
    let accent = roles().sections[1];
    controls::instrument::eq_response_scope(ui, rect, "EQ Curve", &curve, EQ_MAX_DB, eq.on, accent);
    let a11y_name = format!("{name} response curve");
    let response = ui.interact(rect, ui.id().with(&a11y_name), Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, a11y_name.clone())
    });
    name_control(response, &a11y_name);
}

/// Draw the static waveshaper transfer curve for `drive` inside `rect`.
pub(super) fn drive_scope(ui: &mut egui::Ui, rect: Rect, drive: &Drive, track: usize) {
    let curve = sample_drive_curve(drive, DRIVE_POINTS);
    let accent = if drive.on {
        roles().sections[5]
    } else {
        roles().disabled_text
    };
    controls::instrument::scope_frame(ui, rect, "Drive Curve", &curve, accent);
    let label = format!("Track {} drive transfer curve", track + 1);
    let response = ui.interact(rect, ui.id().with(&label), Sense::hover());
    view::name_region(response, &label).hover_hint(
        "Static transfer curve with Mix and Output. Focus and level matching act on the audio.",
    );
}

#[cfg(test)]
#[test]
fn reduced_history_keeps_both_peaks_and_resets_on_a_new_run() {
    let mut history = GrHistory::default();
    let interval = Duration::from_millis(67);
    let both = |compressor_gr_db, ducker_gr_db| TrackMeterReading {
        compressor_gr_db,
        ducker_gr_db,
    };
    let last = GR_HISTORY_LEN - 1;
    history.record(1, both(4.0, 2.0), 0.0, interval);
    history.record(1, both(7.0, 3.0), 0.03, interval);
    history.record(1, both(6.0, 5.0), 0.06, interval);
    assert_eq!(
        [history.samples[0][last], history.samples[1][last]],
        [4.0, 2.0]
    );
    history.record(1, both(1.0, 1.0), 0.07, interval);
    assert_eq!(
        [history.samples[0][last], history.samples[1][last]],
        [7.0, 5.0]
    );
    history.record(2, both(1.5, 0.5), 0.08, interval);
    assert_eq!(
        [history.samples[0][last], history.samples[1][last]],
        [1.5, 0.5]
    );
    assert_eq!(
        [history.samples[0][last - 1], history.samples[1][last - 1]],
        [0.0, 0.0]
    );
    // A window that was not drawn for a while drops its old trace.
    history.record(2, both(2.5, 1.5), 1.0, interval);
    assert_eq!(
        [history.samples[0][last], history.samples[1][last]],
        [2.5, 1.5]
    );
    assert_eq!(
        [history.samples[0][last - 1], history.samples[1][last - 1]],
        [0.0, 0.0]
    );
}
