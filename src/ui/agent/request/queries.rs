//! Agent requests that reply at once: help, the state, sounds, controls, the
//! library folders, Album Reviews, and a playback capture.
use super::*;

impl MixJamApp {
    /// Answer a request that needs no input steps, or start a library scan.
    pub(super) fn agent_query(
        &mut self,
        ctx: &egui::Context,
        request: Request,
    ) -> Result<Start, String> {
        Ok(match request {
            Request::Help => Start::Reply(Body::Help { help: wire::HELP }),
            Request::State => Start::Reply(Body::State(Box::new(self.agent_state(ctx)))),
            Request::Samples {
                search,
                group,
                library,
                offset,
                limit,
            } => self.agent_samples(&search, group.as_deref(), library, offset, limit)?,
            Request::Ui {
                all,
                window,
                contains,
            } => self.agent_ui(ctx, all, window.as_deref(), contains),
            Request::Libraries => Start::Reply(Body::Libraries {
                folders: self.session.library().wav_roots().to_vec(),
                unloaded: wire::unloaded_views(&self.session),
                max_folders: MAX_LIBRARY_FOLDERS,
                fallback_bpm: self.libraries.fallback_bpm,
                scan: wire::busy(&self.session).scan,
            }),
            Request::SetLibraries {
                folders,
                fallback_bpm,
            } => {
                if let Some(modal) = self.blocking_modal(ctx) {
                    return Err(format!(
                        "The library folders cannot change while the {modal} modal is open."
                    ));
                }
                if fallback_bpm.is_some_and(|bpm| !bpm.is_finite() || !(1.0..=999.0).contains(&bpm))
                {
                    return Err("The fallback BPM is 1 to 999.".into());
                }
                self.try_scan_library_folders(
                    folders,
                    fallback_bpm.unwrap_or(self.libraries.fallback_bpm),
                )?;
                Start::run(vec![Step::Idle])
            }
            // The reviews do not depend on the window, its modal, or the song.
            request @ (Request::Reviews { .. }
            | Request::Review { .. }
            | Request::ReviewTracks { .. }
            | Request::AddReview { .. }
            | Request::UpdateReview { .. }) => {
                match wire::album_reply(self.album_review.state.shelf(), &request) {
                    Some(reply) => Start::Reply(reply?),
                    None => return Err("That is not an Album Review request.".into()),
                }
            }
            Request::Capture { reason } => {
                let reason = reason.unwrap_or_else(|| "Saved by an agent".to_owned());
                let (folder, state) = self.session.save_playback_capture(&reason)?;
                Start::Reply(Body::Capture { folder, state })
            }
            _ => return Err("That is not a query request.".into()),
        })
    }

    /// The library sounds whose name contains `search`, by group and library.
    fn agent_samples(
        &self,
        search: &str,
        group: Option<&str>,
        library: Option<String>,
        offset: usize,
        limit: Option<usize>,
    ) -> Result<Start, String> {
        let library = library
            .map(|library| Product::parse(&library))
            .transpose()?
            .map(Product::label);
        let search = search.to_lowercase();
        let found: Vec<_> = self
            .session
            .library()
            .samples
            .iter()
            .filter(|sample| {
                sample.name.to_lowercase().contains(&search)
                    && group.is_none_or(|group| sample.group_name.eq_ignore_ascii_case(group))
            })
            .map(|sample| wire::sample_view(&self.session, sample))
            .filter(|sample| library.is_none() || sample.library == library)
            .collect();
        Ok(Start::Reply(Body::Samples {
            total: found.len(),
            samples: found
                .into_iter()
                .skip(offset)
                .take(limit.unwrap_or(50))
                .collect(),
        }))
    }

    /// The window's controls, in one window and with a text when given.
    fn agent_ui(
        &self,
        ctx: &egui::Context,
        all: bool,
        window: Option<&str>,
        contains: Option<String>,
    ) -> Start {
        let contains = contains.map(|text| text.to_lowercase());
        Start::Reply(Body::Ui {
            modal: self.blocking_modal(ctx),
            nodes: self
                .agent_nodes(ctx)
                .into_iter()
                .filter(|node| {
                    // Without `all`: what a person can operate now.
                    let usable = node.enabled
                        && !matches!(node.role, "label" | "image")
                        && (!node.label.is_empty() || node.text.is_some())
                        && (node.clickable
                            || matches!(
                                node.role,
                                "slider" | "number" | "text_input" | "combo_box" | "window"
                            ));
                    (all || usable)
                        && match window {
                            Some(title) if node.role == "window" => {
                                node.label.eq_ignore_ascii_case(title)
                            }
                            window => in_window(node, window),
                        }
                        && contains.as_ref().is_none_or(|text| {
                            node.label.to_lowercase().contains(text)
                                || node
                                    .text
                                    .as_ref()
                                    .is_some_and(|value| value.to_lowercase().contains(text))
                        })
                })
                .collect(),
        })
    }
}
