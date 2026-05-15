use super::editors;
use super::settings;
use super::state::{AppState, Selection, SettingsSection, Tab};
use super::theme;
use ulid::Ulid;

pub fn show(state: &mut AppState, ctx: &egui::Context) {
    egui::SidePanel::left("tabs_panel")
        .resizable(false)
        .exact_width(180.0)
        .frame(
            egui::Frame::default()
                .fill(theme::SIDEBAR_BG)
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ctx, |ui| {
            tabs(state, ui);
        });

    egui::SidePanel::left("list_panel")
        .resizable(true)
        .default_width(300.0)
        .min_width(200.0)
        .frame(
            egui::Frame::default()
                .fill(theme::PANEL_BG)
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ctx, |ui| {
            list(state, ui);
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        let selection = state.selection.clone();
        egui::ScrollArea::vertical()
            .id_salt("central_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| match selection {
                Selection::Idea(id) => editors::idea_editor(state, ui, id),
                Selection::Category(id) => editors::category_editor(state, ui, id),
                Selection::Story(id) => editors::story_editor(state, ui, id),
                Selection::Settings(SettingsSection::Llm) => settings::llm_settings(state, ui),
                Selection::Settings(SettingsSection::Tts) => settings::tts_settings(state, ui),
                Selection::None => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(
                            egui::RichText::new(
                                "左のリストから項目を選ぶか、新規作成してください。",
                            )
                            .color(theme::SUBTLE_TEXT),
                        );
                    });
                }
            });
    });
}

fn tabs(state: &mut AppState, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    tab_button(ui, state, Tab::Ideas, "アイデア");
    tab_button(ui, state, Tab::Categories, "カテゴリ");
    tab_button(ui, state, Tab::Stories, "ストーリー");
    tab_button(ui, state, Tab::Settings, "設定");

    let avail = ui.available_height();
    if avail > 24.0 {
        ui.add_space(avail - 24.0);
    }
    let path = state
        .config
        .data_dir
        .clone()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    ui.label(
        egui::RichText::new(format!("data: {path}"))
            .small()
            .color(theme::SUBTLE_TEXT),
    );
}

fn tab_button(ui: &mut egui::Ui, state: &mut AppState, tab: Tab, label: &str) {
    let active = state.tab == tab;
    let text = if active {
        egui::RichText::new(label).strong().color(egui::Color32::WHITE)
    } else {
        egui::RichText::new(label).color(egui::Color32::from_rgb(0xc4, 0xc8, 0xd0))
    };
    let btn = egui::Button::new(text)
        .min_size(egui::vec2(ui.available_width(), 32.0))
        .fill(if active {
            egui::Color32::from_rgb(0x23, 0x26, 0x2d)
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::NONE);
    if ui.add(btn).clicked() {
        state.tab = tab;
        state.selection = Selection::None;
    }
}

fn list(state: &mut AppState, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| match state.tab {
        Tab::Ideas => idea_list(state, ui),
        Tab::Categories => category_list(state, ui),
        Tab::Stories => story_list(state, ui),
        Tab::Settings => settings_list(state, ui),
    });
}

fn settings_list(state: &mut AppState, ui: &mut egui::Ui) {
    let llm_selected = matches!(state.selection, Selection::Settings(SettingsSection::Llm));
    list_row(ui, llm_selected, "LLM 接続", "llama.cpp サーバーのエンドポイント等", || {
        state.selection = Selection::Settings(SettingsSection::Llm);
    });
    let tts_selected = matches!(state.selection, Selection::Settings(SettingsSection::Tts));
    list_row(ui, tts_selected, "TTS 接続", "読み上げ用 (OpenAI /audio/speech)", || {
        state.selection = Selection::Settings(SettingsSection::Tts);
    });
}

fn idea_list(state: &mut AppState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        if theme::primary_button(ui, "+ 新規").clicked() {
            if let Ok(id) = state.create_idea() {
                state.selection = Selection::Idea(id);
            }
        }
    });
    ui.separator();

    let category_lookup: std::collections::HashMap<Ulid, String> = state
        .categories
        .iter()
        .map(|c| (c.meta.id, c.meta.name.clone()))
        .collect();
    let items: Vec<(Ulid, String, String)> = state
        .ideas
        .iter()
        .map(|i| {
            let cats: Vec<_> = i
                .meta
                .categories
                .iter()
                .filter_map(|cid| category_lookup.get(cid).cloned())
                .collect();
            (i.meta.id, i.meta.title.clone(), cats.join(" / "))
        })
        .collect();
    let selected_id = match state.selection {
        Selection::Idea(id) => Some(id),
        _ => None,
    };
    for (id, title, sub) in items {
        let selected = selected_id == Some(id);
        list_row(ui, selected, &title, &sub, || {
            state.selection = Selection::Idea(id);
        });
    }
}

fn category_list(state: &mut AppState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        if theme::primary_button(ui, "+ 新規").clicked() {
            if let Ok(id) = state.create_category() {
                state.selection = Selection::Category(id);
            }
        }
    });
    ui.separator();

    let items: Vec<(Ulid, String)> = state
        .categories
        .iter()
        .map(|c| (c.meta.id, c.meta.name.clone()))
        .collect();
    let selected_id = match state.selection {
        Selection::Category(id) => Some(id),
        _ => None,
    };
    for (id, name) in items {
        let selected = selected_id == Some(id);
        list_row(ui, selected, &name, "", || {
            state.selection = Selection::Category(id);
        });
    }
}

fn story_list(state: &mut AppState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        if theme::primary_button(ui, "+ 新規").clicked() {
            if let Ok(id) = state.create_story() {
                state.selection = Selection::Story(id);
            }
        }
    });
    ui.separator();

    let items: Vec<(Ulid, String, String)> = state
        .stories
        .iter()
        .map(|s| {
            let nsfw = if s.meta.nsfw { "NSFW" } else { "SFW" };
            (s.meta.id, s.meta.title.clone(), nsfw.to_string())
        })
        .collect();
    let selected_id = match state.selection {
        Selection::Story(id) => Some(id),
        _ => None,
    };
    for (id, title, sub) in items {
        let selected = selected_id == Some(id);
        list_row(ui, selected, &title, &sub, || {
            state.selection = Selection::Story(id);
        });
    }
}

fn list_row<F: FnMut()>(
    ui: &mut egui::Ui,
    selected: bool,
    title: &str,
    sub: &str,
    mut on_click: F,
) {
    let bg = if selected {
        egui::Color32::from_rgb(0x2e, 0x33, 0x40)
    } else {
        egui::Color32::TRANSPARENT
    };
    let response = egui::Frame::default()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(title));
                if !sub.is_empty() {
                    ui.label(egui::RichText::new(sub).small().color(theme::SUBTLE_TEXT));
                }
            });
        })
        .response
        .interact(egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            response.rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(0x23, 0x26, 0x2d, 80),
        );
    }
    if response.clicked() {
        on_click();
    }
    ui.add_space(1.0);
    ui.separator();
}
