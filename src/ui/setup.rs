use super::state::AppState;
use super::theme;
use crate::config;

pub fn show(state: &mut AppState, ui: &mut egui::Ui) {
    let avail = ui.available_size();
    ui.allocate_ui_with_layout(
        avail,
        egui::Layout::centered_and_justified(egui::Direction::TopDown),
        |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("library-maid へようこそ");
                ui.add_space(8.0);
                ui.label("アイデアと小説の保存先フォルダを選択してください。");
                ui.label(
                    egui::RichText::new(
                        "Markdown ファイル + YAML フロントマターで保存され、git 管理に向いた構成になります。",
                    )
                    .color(theme::SUBTLE_TEXT),
                );
                ui.add_space(16.0);
                if theme::primary_button(ui, "保存先フォルダを選ぶ…").clicked() {
                    state.status = None;
                    if let Some(dir) = config::pick_data_dir() {
                        if let Err(e) = state.set_data_dir(dir) {
                            state.status = Some(format!("設定の保存に失敗: {e}"));
                        }
                    }
                }
            });
        },
    );
}
