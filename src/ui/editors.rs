use super::state::{
    AppState, FillProposal, Proposal, Selection, StoryEditorState, StoryEvent,
};
use super::theme;
use crate::llm::{
    LlmClient, apply_fills, extract_fills, parse_numbered_blocks, resolve_active_ideas,
    strip_author_notes, visible_text,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use ulid::Ulid;

pub fn idea_editor(state: &mut AppState, ui: &mut egui::Ui, id: Ulid) {
    if state.idea(id).is_none() {
        ui.label("アイデアが見つかりません");
        return;
    }

    let mut delete_requested = false;
    let mut title_changed = false;
    let mut body_changed = false;
    let mut commit = false;

    // Header row
    ui.horizontal(|ui| {
        ui.label("タイトル");
        let idea = state.idea_mut(id).unwrap();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut idea.meta.title)
                .desired_width(f32::INFINITY)
                .min_size(egui::vec2(0.0, 24.0)),
        );
        if resp.changed() {
            title_changed = true;
        }
        if resp.lost_focus() {
            commit = true;
        }
        if theme::danger_button(ui, "削除").clicked() {
            delete_requested = true;
        }
    });

    if delete_requested {
        let _ = state.delete_idea(id);
        state.selection = Selection::None;
        return;
    }

    // Categories chips
    let all_categories: Vec<(Ulid, String)> = state
        .categories
        .iter()
        .map(|c| (c.meta.id, c.meta.name.clone()))
        .collect();
    let cats_selected: std::collections::HashSet<Ulid> = state
        .idea(id)
        .map(|i| i.meta.categories.iter().copied().collect())
        .unwrap_or_default();

    ui.horizontal_wrapped(|ui| {
        ui.label("カテゴリ");
        if all_categories.is_empty() {
            ui.label(egui::RichText::new("カテゴリ未作成").color(theme::SUBTLE_TEXT));
        }
        for (cid, name) in &all_categories {
            let on = cats_selected.contains(cid);
            let chip = chip_button(ui, &name, on, false);
            if chip.clicked() {
                if let Some(i) = state.idea_mut(id) {
                    if i.meta.categories.contains(cid) {
                        i.meta.categories.retain(|x| x != cid);
                    } else {
                        i.meta.categories.push(*cid);
                    }
                }
                commit = true;
            }
        }
    });

    // requires (other ideas)
    let all_ideas: Vec<(Ulid, String)> = state
        .ideas
        .iter()
        .filter(|i| i.meta.id != id)
        .map(|i| (i.meta.id, i.meta.title.clone()))
        .collect();
    let req_selected: std::collections::HashSet<Ulid> = state
        .idea(id)
        .map(|i| i.meta.requires.iter().copied().collect())
        .unwrap_or_default();

    ui.horizontal_wrapped(|ui| {
        ui.label("依存");
        if all_ideas.is_empty() {
            ui.label(egui::RichText::new("他のアイデアなし").color(theme::SUBTLE_TEXT));
        }
        for (iid, ititle) in &all_ideas {
            let on = req_selected.contains(iid);
            let chip = chip_button(ui, &ititle, on, false);
            if chip.clicked() {
                if let Some(i) = state.idea_mut(id) {
                    if i.meta.requires.contains(iid) {
                        i.meta.requires.retain(|x| x != iid);
                    } else {
                        i.meta.requires.push(*iid);
                    }
                }
                commit = true;
            }
        }
    });

    ui.separator();

    {
        let idea = state.idea_mut(id).unwrap();
        let resp = ui.add(
            egui::TextEdit::multiline(&mut idea.body)
                .desired_width(f32::INFINITY)
                .desired_rows(20),
        );
        if resp.changed() {
            body_changed = true;
        }
        if resp.lost_focus() {
            commit = true;
        }
    }

    let _ = title_changed;
    let _ = body_changed;
    if commit {
        state.save_idea_now(id);
    }
}

pub fn category_editor(state: &mut AppState, ui: &mut egui::Ui, id: Ulid) {
    if state.category(id).is_none() {
        ui.label("カテゴリが見つかりません");
        return;
    }

    let mut delete_requested = false;
    let mut commit = false;

    ui.horizontal(|ui| {
        ui.label("名前");
        let c = state.category_mut(id).unwrap();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut c.meta.name)
                .desired_width(f32::INFINITY)
                .min_size(egui::vec2(0.0, 24.0)),
        );
        if resp.lost_focus() {
            commit = true;
        }
        if theme::danger_button(ui, "削除").clicked() {
            delete_requested = true;
        }
    });

    if delete_requested {
        let _ = state.delete_category(id);
        state.selection = Selection::None;
        return;
    }

    ui.separator();
    {
        let c = state.category_mut(id).unwrap();
        let resp = ui.add(
            egui::TextEdit::multiline(&mut c.body)
                .desired_width(f32::INFINITY)
                .desired_rows(20),
        );
        if resp.lost_focus() {
            commit = true;
        }
    }

    if commit {
        state.save_category_now(id);
    }
}

fn chip_button(ui: &mut egui::Ui, label: &str, on: bool, auto: bool) -> egui::Response {
    let (fill, text_color) = if auto {
        (
            theme::AUTO_GREEN,
            egui::Color32::from_rgb(0xb9, 0xe8, 0xc8),
        )
    } else if on {
        (theme::ACCENT, egui::Color32::WHITE)
    } else {
        (
            egui::Color32::from_rgb(0x2a, 0x2d, 0x33),
            egui::Color32::from_rgb(0xd0, 0xd4, 0xdc),
        )
    };
    let btn = egui::Button::new(egui::RichText::new(label).color(text_color))
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(12));
    let mut resp = ui.add(btn);
    if auto {
        resp = resp.on_hover_text("依存により自動有効");
    }
    resp
}

// =============================================================================
// Story editor
// =============================================================================

pub fn story_editor(state: &mut AppState, ui: &mut egui::Ui, id: Ulid) {
    if state.story(id).is_none() {
        ui.label("ストーリーが見つかりません");
        return;
    }

    state
        .editors
        .entry(id)
        .or_insert_with(StoryEditorState::new);

    // ----- meta row -----
    let mut delete_requested = false;
    let mut commit_story = false;
    let mut do_title_gen = false;

    let title_busy = state
        .editors
        .get(&id)
        .map(|e| e.title_generating)
        .unwrap_or(false);
    let any_busy = state.editors.get(&id).map(|e| e.busy()).unwrap_or(false);

    ui.horizontal(|ui| {
        let s = state.story_mut(id).unwrap();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut s.meta.title)
                .hint_text("タイトル")
                .min_size(egui::vec2(280.0, 24.0)),
        );
        if resp.lost_focus() {
            commit_story = true;
        }

        let title_label = if title_busy {
            "タイトル生成中…"
        } else {
            "タイトル生成"
        };
        if ui
            .add_enabled(!any_busy, egui::Button::new(title_label))
            .on_hover_text("本文と設定からタイトル候補を生成")
            .clicked()
        {
            do_title_gen = true;
        }

        if ui.checkbox(&mut s.meta.nsfw, "NSFW").changed() {
            commit_story = true;
        }

        let mut disable_thinking = state.config.llm.disable_thinking;
        let resp = ui
            .checkbox(&mut disable_thinking, "Thinking 無効")
            .on_hover_text("reasoning_effort=none を送信。Qwen3 等で思考トークンを抑止");
        if resp.changed() {
            state.config.llm.disable_thinking = disable_thinking;
            if let Err(e) = state.save_config() {
                tracing::error!(?e, "save config");
            }
        }

        if theme::danger_button(ui, "削除").clicked() {
            delete_requested = true;
        }
    });

    if delete_requested {
        let _ = state.delete_story(id);
        state.selection = Selection::None;
        return;
    }
    if commit_story {
        state.save_story_now(id);
    }
    if do_title_gen {
        spawn_title(state, id);
    }

    title_proposals_row(state, ui, id);

    // ----- active idea chips -----
    let chips: Vec<(Ulid, String, bool, bool)> = {
        let s = state.story(id).unwrap();
        let auto = resolve_active_ideas(&state.ideas, &s.meta.active_ideas);
        let auto_ids: std::collections::HashSet<Ulid> =
            auto.iter().map(|i| i.meta.id).collect();
        let manual: std::collections::HashSet<Ulid> =
            s.meta.active_ideas.iter().copied().collect();
        state
            .ideas
            .iter()
            .map(|i| {
                let on = manual.contains(&i.meta.id);
                let auto_only = auto_ids.contains(&i.meta.id) && !on;
                (i.meta.id, i.meta.title.clone(), on, auto_only)
            })
            .collect()
    };

    let mut chip_toggle: Option<Ulid> = None;
    egui::ScrollArea::vertical()
        .id_salt("idea_chips_scroll")
        .max_height(96.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (iid, ititle, on, auto_only) in &chips {
                    let r = chip_button(ui, ititle, *on, *auto_only);
                    if r.clicked() && !*auto_only {
                        chip_toggle = Some(*iid);
                    }
                }
            });
        });
    if let Some(iid) = chip_toggle {
        if let Some(s) = state.story_mut(id) {
            if let Some(pos) = s.meta.active_ideas.iter().position(|x| *x == iid) {
                s.meta.active_ideas.remove(pos);
            } else {
                s.meta.active_ideas.push(iid);
            }
        }
        state.save_story_now(id);
    }

    ui.separator();

    // Body (fixed rows; outer ScrollArea in library.rs handles overflow).
    // Use `.show(ui)` instead of `ui.add(...)` so we can read the selection
    // range out of the TextEditOutput.
    let (lost_focus, selection) = {
        let s = state.story_mut(id).unwrap();
        let output = egui::TextEdit::multiline(&mut s.body)
            .desired_width(f32::INFINITY)
            .desired_rows(18)
            .font(egui::TextStyle::Monospace)
            .show(ui);
        let sel = output
            .cursor_range
            .map(|r| (r.primary.index, r.secondary.index));
        (output.response.lost_focus(), sel)
    };
    if let Some(ed) = state.editors.get_mut(&id) {
        // Only overwrite when egui reports a selection; otherwise keep the
        // last known one so the read-aloud button still works after the
        // TextEdit loses focus.
        if selection.is_some() {
            ed.body_selection = selection;
        }
    }
    if lost_focus {
        state.save_story_now(id);
    }

    tts_panel(state, ui, id);

    ui.separator();

    // Generation panel — flows below; the outer ScrollArea scrolls when the
    // window is too short to fit body + proposals together.
    gen_panel(state, ui, id);
}

fn gen_panel(state: &mut AppState, ui: &mut egui::Ui, id: Ulid) {
    // Snapshot busy flags before borrowing editor mutably for actions.
    let (busy_general, busy_fill, count_now) = {
        let ed = state.editors.get(&id).unwrap();
        (
            ed.generating || ed.auto_running || ed.compacting || ed.title_generating,
            ed.fill_generating,
            ed.count,
        )
    };

    // ----- Controls row -----
    let mut do_generate = false;
    let mut do_auto = false;
    let mut do_compact = false;
    let mut do_fill = false;
    let mut do_cancel = false;
    let mut clear_proposals = false;
    let mut clear_fill_proposals = false;
    let mut insert_note = false;
    let mut insert_fill = false;

    ui.horizontal_wrapped(|ui| {
        if theme::primary_button_enabled(ui, !busy_general, generate_btn_label(state, id))
            .on_hover_text("案を N 個生成して提示。採用したものだけ本文に追加")
            .clicked()
        {
            do_generate = true;
        }

        let auto_label = auto_btn_label(state, id);
        if ui
            .add_enabled(!busy_general, egui::Button::new(auto_label))
            .on_hover_text("N 回連続で生成 → 自動で本文に追加。ボツ案なし")
            .clicked()
        {
            do_auto = true;
        }

        let compact_label = compact_btn_label(state, id);
        if ui
            .add_enabled(!busy_general, egui::Button::new(compact_label))
            .on_hover_text("本文の前半を要約に置き換えてコンテキスト消費を抑える")
            .clicked()
        {
            do_compact = true;
        }

        ui.label("回数:");
        let mut count_val = count_now;
        if ui
            .add(egui::DragValue::new(&mut count_val).range(1..=20).speed(0.1))
            .changed()
        {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.count = count_val.max(1).min(20);
            }
        }

        let fill_label = fill_btn_label(state, id);
        if ui
            .add_enabled(!busy_general && !busy_fill, egui::Button::new(fill_label))
            .on_hover_text("本文中の <!-- FILL: ... --> 全部を1回のAPIで埋める")
            .clicked()
        {
            do_fill = true;
        }

        if busy_general || busy_fill {
            if theme::danger_button(ui, "キャンセル").clicked() {
                do_cancel = true;
            }
        }

        let (proposals_len, fill_len) = {
            let ed = state.editors.get(&id).unwrap();
            (ed.proposals.len(), ed.fill_proposals.len())
        };
        if proposals_len > 0 {
            if ui.button("すべて破棄").clicked() {
                clear_proposals = true;
            }
        }
        if fill_len > 0 {
            if ui.button("穴埋め案を全破棄").clicked() {
                clear_fill_proposals = true;
            }
        }

        if ui
            .button("著者注を挿入")
            .on_hover_text("AIへの指示を本文末尾にHTMLコメントとして挿入")
            .clicked()
        {
            insert_note = true;
        }
        if ui
            .button("FILL を挿入")
            .on_hover_text("本文末尾にプレースホルダー <!-- FILL: ヒント --> を挿入")
            .clicked()
        {
            insert_fill = true;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("案: {proposals_len} / 穴埋め案: {fill_len}"))
                    .small()
                    .color(theme::SUBTLE_TEXT),
            );
        });
    });

    if do_cancel {
        if let Some(ed) = state.editors.get(&id) {
            ed.cancel_flag.store(true, Ordering::SeqCst);
        }
    }
    if clear_proposals {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.proposals.clear();
        }
    }
    if clear_fill_proposals {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.fill_proposals.clear();
        }
    }
    if insert_note {
        if let Some(s) = state.story_mut(id) {
            if !s.body.is_empty() && !s.body.ends_with('\n') {
                s.body.push('\n');
            }
            s.body.push_str("<!-- NOTE: ここに指示を書く -->\n");
        }
        state.save_story_now(id);
    }
    if insert_fill {
        if let Some(s) = state.story_mut(id) {
            if !s.body.is_empty() && !s.body.ends_with('\n') {
                s.body.push('\n');
            }
            s.body.push_str("<!-- FILL: ここを埋める -->\n");
        }
        state.save_story_now(id);
    }

    // Triggers that spawn async work.
    if do_generate {
        spawn_generate(state, id);
    }
    if do_auto {
        spawn_auto(state, id);
    }
    if do_compact {
        spawn_compact(state, id);
    }
    if do_fill {
        spawn_fill(state, id);
    }

    // ----- Status / errors / live previews -----
    let (fill_err, compact_err) = {
        let ed = state.editors.get(&id).unwrap();
        (ed.fill_error.clone(), ed.compact_error.clone())
    };
    let mut clear_fill_err = false;
    let mut clear_compact_err = false;
    if let Some(err) = fill_err {
        if error_banner(ui, &format!("穴埋め失敗: {err}")) {
            clear_fill_err = true;
        }
    }
    if let Some(err) = compact_err {
        if error_banner(ui, &format!("圧縮失敗: {err}")) {
            clear_compact_err = true;
        }
    }
    if clear_fill_err {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.fill_error = None;
        }
    }
    if clear_compact_err {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.compact_error = None;
        }
    }

    // compact live preview
    {
        let (compacting, live) = {
            let ed = state.editors.get(&id).unwrap();
            (ed.compacting, ed.compact_live.clone())
        };
        if compacting {
            let (visible, in_think) = visible_text(&live);
            let visible = visible.trim_start().to_string();
            proposal_card_pending(
                ui,
                "圧縮中: 前半を要約に置換します",
                &visible,
                in_think,
                "要約開始待ち…",
            );
        }
    }

    // auto live preview
    {
        let (running, live) = {
            let ed = state.editors.get(&id).unwrap();
            (ed.auto_running, ed.auto_live.clone())
        };
        if running {
            let (visible, in_think) = visible_text(&live);
            let visible = visible.trim_start().to_string();
            proposal_card_pending(ui, "", &visible, in_think, "生成開始待ち…");
        }
    }

    // proposals
    let proposals = state.editors.get(&id).map(|e| e.proposals.clone()).unwrap_or_default();
    for prop in &proposals {
        proposal_card(state, ui, id, prop);
    }

    // fill proposals
    let fills = state.editors.get(&id).map(|e| e.fill_proposals.clone()).unwrap_or_default();
    for prop in &fills {
        fill_proposal_card(state, ui, id, prop);
    }
}

fn tts_panel(state: &mut AppState, ui: &mut egui::Ui, id: Ulid) {
    let (playing, error, selection) = {
        let Some(ed) = state.editors.get(&id) else { return; };
        (ed.tts_playing, ed.tts_error.clone(), ed.body_selection)
    };
    let has_selection = matches!(selection, Some((a, b)) if a != b);

    let mut do_play = false;
    let mut do_stop = false;
    let mut clear_error = false;

    ui.horizontal(|ui| {
        let label = if has_selection {
            "🔊 選択範囲を読み上げ"
        } else {
            "🔊 カーソル位置の段落を読み上げ"
        };
        if ui
            .add_enabled(!playing, egui::Button::new(label))
            .on_hover_text(
                "選択範囲があれば優先。なければカーソル位置の段落、それも空なら行を読み上げます。",
            )
            .clicked()
        {
            do_play = true;
        }
        if playing {
            if theme::danger_button(ui, "⏹ 停止").clicked() {
                do_stop = true;
            }
            ui.label(
                egui::RichText::new("読み上げ中…")
                    .small()
                    .color(theme::SUBTLE_TEXT),
            );
        }
    });
    if let Some(err) = error {
        if error_banner(ui, &format!("読み上げ失敗: {err}")) {
            clear_error = true;
        }
    }

    if clear_error {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.tts_error = None;
        }
    }
    if do_stop {
        if let Some(ed) = state.editors.get(&id) {
            ed.tts_cancel.store(true, Ordering::SeqCst);
        }
    }
    if do_play {
        spawn_tts(state, id);
    }
}

fn title_proposals_row(state: &mut AppState, ui: &mut egui::Ui, id: Ulid) {
    let (generating, live, candidates, error) = {
        let Some(ed) = state.editors.get(&id) else {
            return;
        };
        (
            ed.title_generating,
            ed.title_live.clone(),
            ed.title_candidates.clone(),
            ed.title_error.clone(),
        )
    };

    if !generating && candidates.is_empty() && error.is_none() {
        return;
    }

    let mut clear_error = false;
    let mut discard_all = false;
    let mut do_cancel = false;
    let mut chosen: Option<String> = None;

    if let Some(err) = error {
        if error_banner(ui, &format!("タイトル生成失敗: {err}")) {
            clear_error = true;
        }
    } else if generating {
        let visible = visible_text(&live).0;
        let preview: String = visible.lines().take(3).collect::<Vec<_>>().join(" / ");
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("タイトル生成中…")
                    .small()
                    .color(theme::SUBTLE_TEXT),
            );
            if !preview.trim().is_empty() {
                ui.label(
                    egui::RichText::new(preview)
                        .small()
                        .color(theme::SUBTLE_TEXT),
                );
            }
            if theme::danger_button(ui, "中止").clicked() {
                do_cancel = true;
            }
        });
    } else if !candidates.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("タイトル候補")
                    .small()
                    .color(theme::SUBTLE_TEXT),
            );
            for cand in &candidates {
                let btn = egui::Button::new(cand)
                    .fill(egui::Color32::from_rgb(0x2a, 0x2d, 0x33))
                    .corner_radius(egui::CornerRadius::same(12));
                let resp = ui
                    .add(btn)
                    .on_hover_text("クリックでタイトルに採用");
                if resp.clicked() {
                    chosen = Some(cand.clone());
                }
            }
            if ui.button("破棄").clicked() {
                discard_all = true;
            }
        });
    }

    if clear_error {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.title_error = None;
        }
    }
    if do_cancel {
        if let Some(ed) = state.editors.get(&id) {
            ed.cancel_flag.store(true, Ordering::SeqCst);
        }
    }
    if let Some(title) = chosen {
        if let Some(s) = state.story_mut(id) {
            s.meta.title = title;
            s.meta.updated_at = chrono::Utc::now();
        }
        state.save_story_now(id);
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.title_candidates.clear();
        }
    }
    if discard_all {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.title_candidates.clear();
        }
    }
}

fn generate_btn_label(state: &AppState, id: Ulid) -> String {
    let ed = state.editors.get(&id).unwrap();
    if ed.generating {
        "生成中…".into()
    } else {
        "続きを生成 (案出し)".into()
    }
}

fn auto_btn_label(state: &AppState, id: Ulid) -> String {
    let ed = state.editors.get(&id).unwrap();
    if ed.auto_running {
        let (done, total) = ed.auto_progress;
        format!("自動連投中… {done}/{total}")
    } else {
        "自動連投".into()
    }
}

fn compact_btn_label(state: &AppState, id: Ulid) -> String {
    let ed = state.editors.get(&id).unwrap();
    if ed.compacting { "圧縮中…".into() } else { "圧縮".into() }
}

fn fill_btn_label(state: &AppState, id: Ulid) -> String {
    let ed = state.editors.get(&id).unwrap();
    if ed.fill_generating {
        "穴埋め中…".into()
    } else {
        "穴埋め生成".into()
    }
}

fn error_banner(ui: &mut egui::Ui, msg: &str) -> bool {
    let mut closed = false;
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(0x3a, 0x1f, 0x22))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0x6b, 0x30, 0x30)))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(msg).color(theme::ERROR_TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("閉じる").clicked() {
                        closed = true;
                    }
                });
            });
        });
    closed
}

fn proposal_card_pending(
    ui: &mut egui::Ui,
    header: &str,
    visible: &str,
    in_think: bool,
    placeholder_when_empty: &str,
) {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(0x1f, 0x22, 0x28))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0x2a, 0x2d, 0x33)))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            if !header.is_empty() {
                ui.label(
                    egui::RichText::new(header).small().color(theme::SUBTLE_TEXT),
                );
            }
            if visible.is_empty() {
                ui.label(
                    egui::RichText::new(if in_think { "考え中…" } else { placeholder_when_empty })
                        .color(theme::SUBTLE_TEXT),
                );
            } else {
                ui.label(visible);
                ui.label(
                    egui::RichText::new(if in_think { " 〔思考中〕" } else { " ▍" })
                        .color(theme::SUBTLE_TEXT),
                );
            }
        });
}

fn proposal_card(state: &mut AppState, ui: &mut egui::Ui, id: Ulid, prop: &Proposal) {
    let pid = prop.id;
    let (visible, in_think) = visible_text(&prop.raw);
    let visible = strip_author_notes(&visible).trim_start().to_string();
    let mut adopt = false;
    let mut discard = false;

    egui::Frame::default()
        .fill(egui::Color32::from_rgb(0x1f, 0x22, 0x28))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0x2a, 0x2d, 0x33)))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            if let Some(err) = prop.error.clone() {
                ui.label(egui::RichText::new(format!("エラー: {err}")).color(theme::ERROR_TEXT));
                ui.horizontal(|ui| {
                    if ui.button("閉じる").clicked() {
                        discard = true;
                    }
                });
                return;
            }

            if visible.is_empty() && prop.pending {
                ui.label(
                    egui::RichText::new(if in_think { "考え中…" } else { "生成開始待ち…" })
                        .color(theme::SUBTLE_TEXT),
                );
            } else {
                ui.label(&visible);
                if prop.pending {
                    ui.label(
                        egui::RichText::new(if in_think { " 〔思考中〕" } else { " ▍" })
                            .color(theme::SUBTLE_TEXT),
                    );
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::primary_button_enabled(ui, !prop.pending && !visible.is_empty(), "採用")
                    .clicked()
                {
                    adopt = true;
                }
                if ui.add_enabled(!prop.pending, egui::Button::new("破棄")).clicked() {
                    discard = true;
                }
            });
        });

    if adopt {
        if let Some(s) = state.story_mut(id) {
            s.append_body(&visible);
        }
        state.save_story_now(id);
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.proposals.retain(|p| p.id != pid);
        }
    } else if discard {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.proposals.retain(|p| p.id != pid);
        }
    }
}

fn fill_proposal_card(
    state: &mut AppState,
    ui: &mut egui::Ui,
    id: Ulid,
    prop: &FillProposal,
) {
    let pid = prop.id;
    let (visible, in_think) = visible_text(&prop.raw);
    let parsed = if prop.pending {
        Default::default()
    } else {
        parse_numbered_blocks(&visible)
    };
    let mut adopt = false;
    let mut discard = false;

    egui::Frame::default()
        .fill(egui::Color32::from_rgb(0x1f, 0x22, 0x28))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0x2a, 0x2d, 0x33)))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("穴埋め案: {} スロット", prop.hints.len()))
                    .small()
                    .color(theme::SUBTLE_TEXT),
            );
            if let Some(err) = prop.error.clone() {
                ui.label(egui::RichText::new(format!("エラー: {err}")).color(theme::ERROR_TEXT));
                if ui.button("閉じる").clicked() {
                    discard = true;
                }
                return;
            }
            if prop.pending {
                if visible.trim().is_empty() {
                    ui.label(
                        egui::RichText::new(if in_think { "考え中…" } else { "生成開始待ち…" })
                            .color(theme::SUBTLE_TEXT),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(format!(
                            "ストリーミング中… ({} 文字受信)",
                            visible.len()
                        ))
                        .color(theme::SUBTLE_TEXT),
                    );
                }
            } else {
                for (i, hint) in prop.hints.iter().enumerate() {
                    let n = i + 1;
                    let filled = parsed.get(&n).cloned().unwrap_or_default();
                    let label = if hint.is_empty() {
                        format!("#{n}")
                    } else {
                        format!("#{n}: {hint}")
                    };
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(label)
                            .small()
                            .color(theme::SUBTLE_TEXT),
                    );
                    if filled.trim().is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "(モデルが #{n} を返さなかった — 採用すると元の FILL マーカーが残ります)"
                            ))
                            .small()
                            .color(theme::ERROR_TEXT),
                        );
                    } else {
                        ui.label(filled);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::primary_button_enabled(ui, !parsed.is_empty(), "採用").clicked() {
                        adopt = true;
                    }
                    if ui.button("破棄").clicked() {
                        discard = true;
                    }
                });
            }
        });

    if adopt {
        if let Some(s) = state.story_mut(id) {
            let slots = extract_fills(&s.body);
            let new = apply_fills(&s.body, &slots, &parsed);
            s.replace_body(new);
        }
        state.save_story_now(id);
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.fill_proposals.retain(|p| p.id != pid);
        }
    } else if discard {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.fill_proposals.retain(|p| p.id != pid);
        }
    }
}

// =============================================================================
// Async spawn helpers
// =============================================================================

fn spawn_generate(state: &mut AppState, id: Ulid) {
    let count;
    let placeholder_pid;
    let cancel;
    let tx;
    {
        let ed = match state.editors.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        if ed.count == 0 || ed.generating {
            return;
        }
        ed.cancel_flag.store(false, Ordering::SeqCst);
        ed.generating = true;
        count = ed.count;
        placeholder_pid = ed.next_pid();
        cancel = ed.cancel_flag.clone();
        tx = ed.tx.clone();
        ed.proposals.push(Proposal {
            id: placeholder_pid,
            raw: String::new(),
            pending: true,
            error: None,
        });
    }
    let snapshot_story = match state.story(id) {
        Some(s) => s.clone(),
        None => return,
    };
    let snapshot_ideas = state.ideas.clone();
    let llm_cfg = state.config.llm.clone();
    let prompts_dir = state
        .library
        .as_ref()
        .map(|l| l.prompts_dir())
        .unwrap_or_default();
    let ctx = state.egui_ctx.clone();

    state.rt.spawn(async move {
        tracing::info!(count, "stream_proposals: spawn start");
        let client = LlmClient::new(llm_cfg);
        let tx2 = tx.clone();
        let ctx2 = ctx.clone();
        let on_delta = move |chunk: &str| {
            let _ = tx2.send(StoryEvent::ProposalDelta {
                pid: placeholder_pid,
                chunk: chunk.to_string(),
            });
            ctx2.request_repaint();
        };
        let result = client
            .stream_proposals(
                &prompts_dir,
                &snapshot_story,
                &snapshot_ideas,
                count,
                cancel.clone(),
                on_delta,
            )
            .await;
        tracing::info!(ok = result.is_ok(), "stream_proposals: finished");
        if cancel.load(Ordering::SeqCst) {
            let _ = tx.send(StoryEvent::ProposalsCancelled { placeholder_pid });
        } else {
            match result {
                Ok(raw) => {
                    let _ = tx.send(StoryEvent::ProposalsDone {
                        placeholder_pid,
                        raw,
                    });
                }
                Err(e) => {
                    let _ = tx.send(StoryEvent::ProposalsError {
                        placeholder_pid,
                        error: format!("{e}"),
                    });
                }
            }
        }
        ctx.request_repaint();
    });
}

fn spawn_auto(state: &mut AppState, id: Ulid) {
    let count;
    let cancel;
    let tx;
    {
        let ed = match state.editors.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        if ed.count == 0 || ed.auto_running || ed.generating {
            return;
        }
        ed.cancel_flag.store(false, Ordering::SeqCst);
        ed.auto_running = true;
        ed.auto_progress = (0, ed.count);
        ed.auto_live.clear();
        count = ed.count;
        cancel = ed.cancel_flag.clone();
        tx = ed.tx.clone();
    }
    let snapshot_story = match state.story(id) {
        Some(s) => s.clone(),
        None => return,
    };
    let snapshot_ideas = state.ideas.clone();
    let llm_cfg = state.config.llm.clone();
    let prompts_dir = state
        .library
        .as_ref()
        .map(|l| l.prompts_dir())
        .unwrap_or_default();
    let ctx = state.egui_ctx.clone();

    state.rt.spawn(async move {
        let client = LlmClient::new(llm_cfg);
        let tx2 = tx.clone();
        let ctx2 = ctx.clone();
        let on_delta = move |chunk: &str| {
            let _ = tx2.send(StoryEvent::AutoDelta {
                chunk: chunk.to_string(),
            });
            ctx2.request_repaint();
        };
        let result = client
            .stream_auto_batch(
                &prompts_dir,
                &snapshot_story,
                &snapshot_ideas,
                count,
                cancel.clone(),
                on_delta,
            )
            .await;
        let cancelled = cancel.load(Ordering::SeqCst);
        match result {
            Ok(raw) => {
                let _ = tx.send(StoryEvent::AutoDone { raw, cancelled });
            }
            Err(e) => {
                let _ = tx.send(StoryEvent::AutoError {
                    error: format!("{e:#}"),
                });
            }
        }
        ctx.request_repaint();
    });
}

fn spawn_compact(state: &mut AppState, id: Ulid) {
    let cancel;
    let tx;
    {
        let ed = match state.editors.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        if ed.compacting || ed.generating || ed.auto_running {
            return;
        }
        ed.cancel_flag.store(false, Ordering::SeqCst);
        ed.compacting = true;
        ed.compact_error = None;
        ed.compact_live.clear();
        cancel = ed.cancel_flag.clone();
        tx = ed.tx.clone();
    }
    let snapshot_story = match state.story(id) {
        Some(s) => s.clone(),
        None => return,
    };
    let snapshot_ideas = state.ideas.clone();
    let llm_cfg = state.config.llm.clone();
    let prompts_dir = state
        .library
        .as_ref()
        .map(|l| l.prompts_dir())
        .unwrap_or_default();
    let ctx = state.egui_ctx.clone();

    state.rt.spawn(async move {
        let client = LlmClient::new(llm_cfg);
        let tx2 = tx.clone();
        let ctx2 = ctx.clone();
        let on_delta = move |chunk: &str| {
            let _ = tx2.send(StoryEvent::CompactDelta {
                chunk: chunk.to_string(),
            });
            ctx2.request_repaint();
        };
        let result = client
            .compact_body(
                &prompts_dir,
                &snapshot_story,
                &snapshot_ideas,
                cancel.clone(),
                on_delta,
            )
            .await;
        match result {
            Ok(new_body) => {
                let _ = tx.send(StoryEvent::CompactDone { new_body });
            }
            Err(e) => {
                let _ = tx.send(StoryEvent::CompactError {
                    error: format!("{e:#}"),
                });
            }
        }
        ctx.request_repaint();
    });
}

fn spawn_fill(state: &mut AppState, id: Ulid) {
    let count;
    let cancel;
    let tx;
    let snapshot_story = match state.story(id) {
        Some(s) => s.clone(),
        None => return,
    };
    let slots = extract_fills(&snapshot_story.body);
    if slots.is_empty() {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.fill_error = Some(
                "本文に <!-- FILL: ... --> がありません。「FILL を挿入」で穴を作ってください。"
                    .to_string(),
            );
        }
        return;
    }

    {
        let ed = match state.editors.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        if ed.count == 0 || ed.busy() {
            return;
        }
        ed.cancel_flag.store(false, Ordering::SeqCst);
        ed.fill_generating = true;
        ed.fill_error = None;
        count = ed.count;
        cancel = ed.cancel_flag.clone();
        tx = ed.tx.clone();
    }

    let snapshot_ideas = state.ideas.clone();
    let llm_cfg = state.config.llm.clone();
    let prompts_dir = state
        .library
        .as_ref()
        .map(|l| l.prompts_dir())
        .unwrap_or_default();
    let ctx = state.egui_ctx.clone();
    let hints: Vec<String> = slots.iter().map(|s| s.hint.clone()).collect();

    // Allocate placeholder PIDs up front so they show immediately in the UI.
    let mut pids = Vec::with_capacity(count as usize);
    if let Some(ed) = state.editors.get_mut(&id) {
        for _ in 0..count {
            let pid = ed.next_pid();
            pids.push(pid);
            ed.fill_proposals.push(FillProposal {
                id: pid,
                raw: String::new(),
                pending: true,
                error: None,
                hints: hints.clone(),
            });
        }
    }

    state.rt.spawn(async move {
        let client = LlmClient::new(llm_cfg);
        for pid in pids {
            if cancel.load(Ordering::SeqCst) {
                let _ = tx.send(StoryEvent::FillCancelled { pid });
                continue;
            }
            let tx2 = tx.clone();
            let ctx2 = ctx.clone();
            let on_delta = move |chunk: &str| {
                let _ = tx2.send(StoryEvent::FillDelta {
                    pid,
                    chunk: chunk.to_string(),
                });
                ctx2.request_repaint();
            };
            let result = client
                .stream_fill(
                    &prompts_dir,
                    &snapshot_story,
                    &snapshot_ideas,
                    &slots,
                    cancel.clone(),
                    on_delta,
                )
                .await;
            if cancel.load(Ordering::SeqCst) {
                let _ = tx.send(StoryEvent::FillCancelled { pid });
                continue;
            }
            match result {
                Ok(raw) => {
                    let _ = tx.send(StoryEvent::FillDone { pid, raw });
                }
                Err(e) => {
                    let _ = tx.send(StoryEvent::FillError {
                        pid,
                        error: format!("{e:#}"),
                    });
                }
            }
            ctx.request_repaint();
        }
        // Mark generation done. We piggy-back on FillCancelled when no events
        // remain — simpler: send a sentinel via FillError on a non-existent PID
        // or use a dedicated event. Use a dedicated approach: drain side
        // detects no in-flight pids and clears `fill_generating`.
        let _ = tx.send(StoryEvent::FillCancelled { pid: 0 });
        ctx.request_repaint();
    });
}

fn spawn_tts(state: &mut AppState, id: Ulid) {
    let selection = state.editors.get(&id).and_then(|ed| ed.body_selection);
    let cursor = selection.map(|(primary, _)| primary);
    let text = state
        .story(id)
        .and_then(|s| crate::tts::pick_speech_text(&s.body, selection, cursor));
    let Some(text) = text else {
        if let Some(ed) = state.editors.get_mut(&id) {
            ed.tts_error = Some(
                "読み上げ対象がありません。本文に文章を書くか、範囲を選択してください。".into(),
            );
        }
        return;
    };

    let tx;
    let cancel;
    {
        let Some(ed) = state.editors.get_mut(&id) else {
            return;
        };
        if ed.tts_playing {
            return;
        }
        ed.tts_playing = true;
        ed.tts_error = None;
        // Fresh cancel flag per playback so a previous "stopped" state doesn't
        // immediately abort the new one.
        ed.tts_cancel = Arc::new(AtomicBool::new(false));
        tx = ed.tx.clone();
        cancel = ed.tts_cancel.clone();
    }

    let tts_cfg = state.config.tts.clone();
    let ctx = state.egui_ctx.clone();

    state.rt.spawn(async move {
        tracing::info!(chars = text.chars().count(), "tts: synthesize start");
        let bytes = match tts_cfg.synthesize(&text).await {
            Ok(b) => b,
            Err(e) => {
                let _ = tx.send(StoryEvent::TtsError {
                    error: format!("{e:#}"),
                });
                ctx.request_repaint();
                return;
            }
        };
        let cancel2 = cancel.clone();
        let play_res = tokio::task::spawn_blocking(move || {
            crate::tts::play_blocking(bytes, cancel2)
        })
        .await;
        match play_res {
            Ok(Ok(())) => {
                let _ = tx.send(StoryEvent::TtsDone);
            }
            Ok(Err(e)) => {
                let _ = tx.send(StoryEvent::TtsError {
                    error: format!("{e:#}"),
                });
            }
            Err(e) => {
                let _ = tx.send(StoryEvent::TtsError {
                    error: format!("playback task panicked: {e}"),
                });
            }
        }
        ctx.request_repaint();
    });
}

fn spawn_title(state: &mut AppState, id: Ulid) {
    let count;
    let cancel;
    let tx;
    {
        let ed = match state.editors.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        if ed.busy() {
            return;
        }
        ed.cancel_flag.store(false, Ordering::SeqCst);
        ed.title_generating = true;
        ed.title_live.clear();
        ed.title_candidates.clear();
        ed.title_error = None;
        count = ed.count.max(1);
        cancel = ed.cancel_flag.clone();
        tx = ed.tx.clone();
    }
    let snapshot_story = match state.story(id) {
        Some(s) => s.clone(),
        None => return,
    };
    let snapshot_ideas = state.ideas.clone();
    let llm_cfg = state.config.llm.clone();
    let prompts_dir = state
        .library
        .as_ref()
        .map(|l| l.prompts_dir())
        .unwrap_or_default();
    let ctx = state.egui_ctx.clone();

    state.rt.spawn(async move {
        let client = LlmClient::new(llm_cfg);
        let tx2 = tx.clone();
        let ctx2 = ctx.clone();
        let on_delta = move |chunk: &str| {
            let _ = tx2.send(StoryEvent::TitleDelta {
                chunk: chunk.to_string(),
            });
            ctx2.request_repaint();
        };
        let result = client
            .stream_titles(
                &prompts_dir,
                &snapshot_story,
                &snapshot_ideas,
                count,
                cancel.clone(),
                on_delta,
            )
            .await;
        if cancel.load(Ordering::SeqCst) {
            let _ = tx.send(StoryEvent::TitleCancelled);
        } else {
            match result {
                Ok(raw) => {
                    let _ = tx.send(StoryEvent::TitleDone { raw });
                }
                Err(e) => {
                    let _ = tx.send(StoryEvent::TitleError {
                        error: format!("{e:#}"),
                    });
                }
            }
        }
        ctx.request_repaint();
    });
}

// =============================================================================
// Drain async events into UI state
// =============================================================================

pub fn drain_all_editor_events(state: &mut AppState) {
    let ids: Vec<Ulid> = state.editors.keys().copied().collect();
    for id in ids {
        // We pull the receiver out with mem::replace to avoid holding a mutable
        // borrow on `state.editors` while also mutating other AppState fields
        // (e.g. story body).
        let mut events = Vec::new();
        if let Some(ed) = state.editors.get_mut(&id) {
            while let Ok(ev) = ed.rx.try_recv() {
                events.push(ev);
            }
        }
        for ev in events {
            apply_event(state, id, ev);
        }
    }
}

fn apply_event(state: &mut AppState, id: Ulid, ev: StoryEvent) {
    match ev {
        StoryEvent::ProposalDelta { pid, chunk } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                if let Some(p) = ed.proposals.iter_mut().find(|p| p.id == pid) {
                    p.raw.push_str(&chunk);
                }
            }
        }
        StoryEvent::ProposalsDone {
            placeholder_pid,
            raw,
        } => {
            let visible = visible_text(&raw).0;
            let blocks = parse_numbered_blocks(&visible);
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.proposals.retain(|p| p.id != placeholder_pid);
                if blocks.is_empty() {
                    let pid = ed.next_pid();
                    ed.proposals.push(Proposal {
                        id: pid,
                        raw,
                        pending: false,
                        error: None,
                    });
                } else {
                    let mut keys: Vec<usize> = blocks.keys().copied().collect();
                    keys.sort();
                    for k in keys {
                        let pid = ed.next_pid();
                        ed.proposals.push(Proposal {
                            id: pid,
                            raw: blocks[&k].clone(),
                            pending: false,
                            error: None,
                        });
                    }
                }
                ed.generating = false;
            }
        }
        StoryEvent::ProposalsError {
            placeholder_pid,
            error,
        } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                if let Some(p) = ed.proposals.iter_mut().find(|p| p.id == placeholder_pid) {
                    p.pending = false;
                    p.error = Some(error);
                    p.raw.clear();
                }
                ed.generating = false;
            }
        }
        StoryEvent::ProposalsCancelled { placeholder_pid } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.proposals.retain(|p| p.id != placeholder_pid);
                ed.generating = false;
            }
        }

        StoryEvent::AutoDelta { chunk } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.auto_live.push_str(&chunk);
            }
        }
        StoryEvent::AutoDone { raw, cancelled } => {
            let visible = visible_text(&raw).0;
            let blocks = parse_numbered_blocks(&visible);
            let mut keys: Vec<usize> = blocks.keys().copied().collect();
            keys.sort();
            let mut appended = 0u32;
            if keys.is_empty() && !cancelled {
                let prose = strip_author_notes(&visible).trim().to_string();
                if !prose.is_empty() {
                    if let Some(s) = state.story_mut(id) {
                        s.append_body(&prose);
                    }
                    state.save_story_now(id);
                    appended = 1;
                }
            } else {
                for k in keys {
                    let prose = strip_author_notes(&blocks[&k]).trim().to_string();
                    if prose.is_empty() {
                        continue;
                    }
                    if let Some(s) = state.story_mut(id) {
                        s.append_body(&prose);
                    }
                    state.save_story_now(id);
                    appended += 1;
                    if let Some(ed) = state.editors.get_mut(&id) {
                        ed.auto_progress = (appended, ed.count);
                    }
                }
            }
            if appended == 0 {
                tracing::warn!("auto mode: no blocks parsed from response");
            }
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.auto_progress = (ed.count, ed.count);
                ed.auto_live.clear();
                ed.auto_running = false;
            }
        }
        StoryEvent::AutoError { error } => {
            tracing::error!(error = %error, "auto mode generation failed");
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.auto_running = false;
                ed.auto_live.clear();
            }
        }

        StoryEvent::CompactDelta { chunk } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.compact_live.push_str(&chunk);
            }
        }
        StoryEvent::CompactDone { new_body } => {
            if let Some(s) = state.story_mut(id) {
                s.replace_body(new_body);
            }
            state.save_story_now(id);
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.compact_live.clear();
                ed.compacting = false;
            }
        }
        StoryEvent::CompactError { error } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.compact_error = Some(error);
                ed.compact_live.clear();
                ed.compacting = false;
            }
        }

        StoryEvent::FillDelta { pid, chunk } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                if let Some(p) = ed.fill_proposals.iter_mut().find(|p| p.id == pid) {
                    p.raw.push_str(&chunk);
                }
            }
        }
        StoryEvent::FillDone { pid, raw } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                if let Some(p) = ed.fill_proposals.iter_mut().find(|p| p.id == pid) {
                    p.raw = raw;
                    p.pending = false;
                }
            }
            update_fill_generating_flag(state, id);
        }
        StoryEvent::FillError { pid, error } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                if let Some(p) = ed.fill_proposals.iter_mut().find(|p| p.id == pid) {
                    p.pending = false;
                    p.error = Some(error);
                }
            }
            update_fill_generating_flag(state, id);
        }
        StoryEvent::FillCancelled { pid } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                if pid != 0 {
                    ed.fill_proposals.retain(|p| p.id != pid);
                }
            }
            update_fill_generating_flag(state, id);
        }

        StoryEvent::TitleDelta { chunk } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.title_live.push_str(&chunk);
            }
        }
        StoryEvent::TitleDone { raw } => {
            let visible = visible_text(&raw).0;
            let blocks = parse_numbered_blocks(&visible);
            let mut keys: Vec<usize> = blocks.keys().copied().collect();
            keys.sort();
            let mut titles: Vec<String> = keys
                .iter()
                .filter_map(|k| {
                    let body = strip_author_notes(&blocks[k]);
                    body.lines()
                        .map(|l| l.trim())
                        .find(|l| !l.is_empty())
                        .map(|l| l.to_string())
                })
                .collect();
            // Fallback: if the model ignored the format, take the first non-empty line.
            if titles.is_empty() {
                if let Some(line) = strip_author_notes(&visible)
                    .lines()
                    .map(|l| l.trim())
                    .find(|l| !l.is_empty())
                {
                    titles.push(line.to_string());
                }
            }
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.title_candidates = titles;
                ed.title_live.clear();
                ed.title_generating = false;
                if ed.title_candidates.is_empty() {
                    ed.title_error =
                        Some("候補を解析できませんでした。プロンプトやモデルを確認してください".into());
                }
            }
        }
        StoryEvent::TitleError { error } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.title_error = Some(error);
                ed.title_live.clear();
                ed.title_generating = false;
            }
        }
        StoryEvent::TitleCancelled => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.title_live.clear();
                ed.title_candidates.clear();
                ed.title_generating = false;
            }
        }

        StoryEvent::TtsDone => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.tts_playing = false;
            }
        }
        StoryEvent::TtsError { error } => {
            if let Some(ed) = state.editors.get_mut(&id) {
                ed.tts_playing = false;
                ed.tts_error = Some(error);
            }
        }
    }
}

fn update_fill_generating_flag(state: &mut AppState, id: Ulid) {
    if let Some(ed) = state.editors.get_mut(&id) {
        let any_pending = ed.fill_proposals.iter().any(|p| p.pending);
        if !any_pending {
            ed.fill_generating = false;
        }
    }
}
