mod editors;
mod library;
mod settings;
mod setup;
mod state;
mod theme;

pub use state::AppState;

use state::Selection;

pub struct App {
    state: AppState,
    // Keep the runtime alive for the lifetime of the App. Dropping the Runtime
    // shuts down its worker threads, so spawned tasks would silently never run.
    _rt: tokio::runtime::Runtime,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: tokio::runtime::Runtime) -> Self {
        theme::install(&cc.egui_ctx);
        let handle = rt.handle().clone();
        Self {
            state: AppState::new(handle, cc.egui_ctx.clone()),
            _rt: rt,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain async events from all editor channels and the settings probe.
        editors::drain_all_editor_events(&mut self.state);
        settings::drain_settings_events(&mut self.state);

        if self.state.library.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                setup::show(&mut self.state, ui);
            });
        } else {
            library::show(&mut self.state, ctx);
        }

        // Status bar at the bottom (rendered in library::show, but for the setup
        // path we just skip it).
        if let Some(msg) = self.state.status.clone() {
            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(msg).small().weak());
                    if ui.small_button("×").clicked() {
                        self.state.status = None;
                    }
                });
            });
        }

        // Force redraws while any story is generating (so streaming chunks
        // appear without input). Cheap when idle: no editor is busy.
        let any_busy = self.state.editors.values().any(|e| e.busy())
            || matches!(self.state.settings.probe, state::ProbeState::Running);
        if any_busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        // Allow other Selection variants (we use them in match below) — silence
        // the unused-warning for now.
        let _ = Selection::None;
    }
}
