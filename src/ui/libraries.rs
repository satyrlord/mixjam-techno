//! Sample-library management inside Options. These actions use Session
//! immediately and are independent of the editable options draft.
use super::*;

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

impl MixJamApp {
    pub(super) fn sample_libraries_page(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sample libraries");
        ui.label(
            "Folder and provider actions apply separately. Apply and \
             Cancel affect options only.",
        );
        let scan = self.session.wave_scan();
        let folders = self.session.library().wav_roots().to_vec();
        if scan.is_none() && folders.is_empty() {
            ui.label(format!(
                "Add a folder of WAV files to fill the Sounds panel. \
        Subfolders are included. Up to {MAX_LIBRARY_FOLDERS} library \
        folders share the browser. Fallback BPM applies only to \
        sounds whose tempo cannot be recognized."
            ));
        }
        let idle = !self.session.is_recording() && scan.is_none();
        let busy = if scan.is_some() {
            "Wait for the current scan to finish or cancel it."
        } else {
            "Stop recording before you change the library folders."
        };
        ui.horizontal_wrapped(|ui| {
            ui.label("Fallback BPM");
            let fallback =
                controls::number(ui, &mut self.fallback_bpm, 1.0..=999.0, "Fallback BPM")
                    .hover_hint(
                        "Recognized product audio keeps its original tempo. Folders \
        use matching original audio when enough sources agree, then \
        loop lengths, then this fallback. Unresolved half/double \
        matches prefer 88–176 BPM unless the fallback selects the \
        other tempo.",
                    );
            options::remember_focus(&fallback);
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
        ui.small("Used by Add library folder and Scan again.");
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
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Library {}", index + 1));
                let reason = if folders.len() > 1 {
                    busy
                } else {
                    "Add another library folder before you remove this one."
                };
                if library_button(
                    ui,
                    "Remove",
                    &format!("Remove library {}", index + 1),
                    idle && folders.len() > 1,
                    reason,
                )
                .clicked()
                {
                    remove = Some(index);
                }
            });
            let path = display_paths(&folder.to_string_lossy());
            ui.add(egui::Label::new(&path).truncate()).hover_hint(&path);
            if let Some((_, error)) = library
                .unloaded_roots()
                .iter()
                .find(|(root, _)| root == folder)
            {
                ui.colored_label(CORAL, format!("Not loaded: {error}"));
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
                        .selected_text(self.library_product.label())
                        .show_ui(ui, |ui| {
                            for product in Product::ALL {
                                ui.selectable_value(
                                    &mut self.library_product,
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
                        self.ask_path(FileRequest::ProviderFolder(self.library_product));
                    }
                });
                ui.separator();
                let bindings = self.session.library().bindings().to_vec();
                if bindings.is_empty() {
                    ui.label("No providers");
                }
                for binding in bindings {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(binding.product.label());
                        if library_button(
                            ui,
                            "Remove",
                            &format!("Remove {} provider", binding.product.label()),
                            idle,
                            busy,
                        )
                        .clicked()
                        {
                            self.remove_library(binding.product);
                        }
                    });
                    let path = display_paths(&binding.root.to_string_lossy());
                    ui.add(egui::Label::new(&path).truncate()).hover_hint(&path);
                    ui.add_space(6.0);
                }
            });
        options::remember_focus(&providers.header_response);
    }
}
