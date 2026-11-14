//! Sample-library management inside Options. These actions use Session
//! immediately and are independent of the editable options draft.
use super::*;

/// The tempo for WAVs whose own tempo cannot be recognized.
const FALLBACK_TEMPO: &str = "Tempo for files without one";

/// The sample libraries: the provider to add, the scan tempo, and the scan's
/// status line.
pub(super) struct LibrariesState {
    pub(super) product: Product,
    /// The tempo for unrecognized WAVs in the next sample folder scan.
    pub(super) fallback_bpm: f64,
    /// Close untouched startup Options after its scan fills the library.
    pub(super) close_options_after_scan: bool,
    /// The scan progress line last written to the status, while a scan runs.
    pub(super) scan_status: Option<String>,
}

impl Default for LibrariesState {
    fn default() -> Self {
        Self {
            product: Product::T1,
            fallback_bpm: 140.0,
            close_options_after_scan: false,
            scan_status: None,
        }
    }
}

fn library_button(
    ui: &mut egui::Ui,
    label: &str,
    name: &str,
    enabled: bool,
    reason: &str,
) -> Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            controls::button(ui, label, controls::ButtonRole::Secondary)
        })
        .inner;
    let response = name_control(response, name).disabled_hint(reason);
    options::remember_focus(&response);
    response
}

/// A focused Remove action also announces the folder it removes. A disabled
/// one keeps its reason instead.
fn describe_path(response: &Response, path: &str) {
    if response.enabled() {
        response
            .ctx
            .accesskit_node_builder(response.id, |node| node.set_description(path));
    }
}

impl MixJamApp {
    pub(super) fn sample_libraries_page(&mut self, ui: &mut egui::Ui) {
        options::section(ui, "Sample libraries");
        options::help(ui, "Changes here take effect right away.");
        let scan = self.session.wave_scan();
        let folders = self.session.library().wav_roots().to_vec();
        if scan.is_none() && folders.is_empty() {
            ui.label("Add a folder of WAV files to fill the Sounds panel.");
        }
        let idle = !self.session.is_recording() && scan.is_none();
        let busy = if scan.is_some() {
            "Wait for the current scan to finish or cancel it."
        } else {
            "Stop recording before you change the library folders."
        };
        ui.horizontal_wrapped(|ui| {
            let room = folders.len() < MAX_LIBRARY_FOLDERS;
            let full = format!(
                "The browser holds up to {MAX_LIBRARY_FOLDERS} library \
        folders. Remove one first."
            );
            if library_button(
                ui,
                "Add library folder",
                "Add library folder",
                idle && room,
                if room { busy } else { &full },
            )
            .clicked()
            {
                self.ask_path(FileRequest::LibraryFolder);
            }
            if !folders.is_empty()
                && library_button(ui, "Scan again", "Scan again", idle, busy)
                    .hover_hint("Read every library folder again.")
                    .clicked()
            {
                self.scan_library_folders(folders.clone());
            }
        });
        options::help(
            ui,
            format!("Subfolders are included, up to {MAX_LIBRARY_FOLDERS} folders."),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(FALLBACK_TEMPO);
            let fallback = controls::number(
                ui,
                &mut self.libraries.fallback_bpm,
                1.0..=999.0,
                FALLBACK_TEMPO,
            )
            .hover_hint(
                "Recognized product audio keeps its original tempo. Folders \
        use matching original audio when enough sources agree, then \
        loop lengths, then this tempo. Unresolved half/double \
        matches prefer 88–176 BPM unless this tempo selects the \
        other one.",
            );
            options::remember_focus(&fallback);
            ui.label("BPM");
        });
        options::help(ui, "Applies to the next Add library folder or Scan again.");
        if let Some(scan) = &scan {
            ui.horizontal_wrapped(|ui| {
                let (text, fraction) = if scan.files_total == 0 {
                    ("Listing files…".to_string(), None)
                } else {
                    (
                        format!("{} of {} WAVs", scan.files_done, scan.files_total),
                        Some(scan.files_done as f32 / scan.files_total as f32),
                    )
                };
                let width = (ui.available_width() - 170.0).clamp(80.0, 300.0);
                controls::progress(ui, &text, fraction, width);
                if library_button(ui, "Cancel library scan", "Cancel library scan", true, "")
                    .clicked()
                {
                    self.session.cancel_wave_scan();
                    self.sync_notice();
                }
            });
            ui.small(format!("Scanning {}", scan_subject(&scan.roots)));
        }
        let library = self.session.library();
        let mut remove = None;
        for (index, folder) in folders.iter().enumerate() {
            let path = display_paths(&folder.to_string_lossy());
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Library {}", index + 1));
                let reason = if folders.len() > 1 {
                    busy
                } else {
                    "Add another library folder before you remove this one."
                };
                let response = library_button(
                    ui,
                    "Remove",
                    &format!("Remove library {}", index + 1),
                    idle && folders.len() > 1,
                    reason,
                );
                describe_path(&response, &path);
                if response.clicked() {
                    remove = Some(index);
                }
            });
            // Paths wrap in full; nothing depends on a hover.
            ui.add(egui::Label::new(&path).wrap());
            if let Some((_, error)) = library
                .unloaded_roots()
                .iter()
                .find(|(root, _)| root == folder)
            {
                ui.colored_label(roles().coral, format!("Not loaded: {error}"));
            }
            ui.add_space(4.0);
        }
        if !library.folder_tempos().is_empty() {
            let tempos =
                controls::disclosure(format!("Folder tempos ({})", library.folder_tempos().len()))
                    .default_open(self.capture_view_is("options-library-tempos"))
                    .show(ui, |ui| {
                        for (folder, bpm, count) in library.folder_tempos() {
                            let name = library_relative(library, folder);
                            ui.small(format!("{name}: {bpm} BPM, {count} WAVs"));
                        }
                    });
            options::remember_focus(&tempos.header_response);
        }
        if !library.skipped_waves().is_empty() {
            let skipped =
                controls::disclosure(format!("Skipped WAVs ({})", library.skipped_waves().len()))
                    .show(ui, |ui| {
                        for (path, error) in library.skipped_waves() {
                            ui.small(format!("{}: {error}", library_relative(library, path)));
                        }
                    });
            options::remember_focus(&skipped.header_response);
        }
        if let Some(index) = remove {
            self.remove_library_folder(index);
        }
        let providers = controls::disclosure("Optional product providers")
            .default_open(self.capture_view_is("options-library-providers"))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let products = egui::ComboBox::from_id_salt("library_product")
                        .selected_text(self.libraries.product.label())
                        .show_ui(ui, |ui| {
                            for product in Product::ALL {
                                ui.selectable_value(
                                    &mut self.libraries.product,
                                    product,
                                    product.label(),
                                );
                            }
                        })
                        .response;
                    let products = name_control(products, "Provider product");
                    options::remember_focus(&products);
                    if library_button(ui, "Choose folder", "Choose provider folder", idle, busy)
                        .clicked()
                    {
                        self.ask_path(FileRequest::ProviderFolder(self.libraries.product));
                    }
                });
                ui.separator();
                let bindings = self.session.library().bindings().to_vec();
                if bindings.is_empty() {
                    ui.label("No providers");
                }
                for binding in bindings {
                    let path = display_paths(&binding.root.to_string_lossy());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(binding.product.label());
                        let response = library_button(
                            ui,
                            "Remove",
                            &format!("Remove {} provider", binding.product.label()),
                            idle,
                            busy,
                        );
                        describe_path(&response, &path);
                        if response.clicked() {
                            self.remove_library(binding.product);
                        }
                    });
                    ui.add(egui::Label::new(&path).wrap());
                    ui.add_space(6.0);
                }
            });
        options::remember_focus(&providers.header_response);
    }
}
