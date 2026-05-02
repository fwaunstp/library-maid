use super::state::{AppState, Selection};
use crate::data::frontmatter::FrontmatterDoc;
use crate::llm::{
    LlmClient, apply_fills, extract_fills, parse_fill_response, resolve_active_ideas,
    strip_author_notes, visible_text,
};
use chrono::Utc;
use dioxus::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use ulid::Ulid;

fn save_idea(mut state: Signal<AppState>, id: Ulid) {
    if let Some(idea) = state.write().idea_mut(id) {
        idea.meta.updated_at = Utc::now();
        if let Err(e) = idea.save() {
            tracing::error!(?e, "save idea");
        }
    }
}

fn save_category(mut state: Signal<AppState>, id: Ulid) {
    if let Some(c) = state.write().category_mut(id) {
        if let Err(e) = c.save() {
            tracing::error!(?e, "save category");
        }
    }
}

fn save_story(mut state: Signal<AppState>, id: Ulid) {
    if let Some(s) = state.write().story_mut(id) {
        if let Err(e) = s.save() {
            tracing::error!(?e, "save story");
        }
    }
}

#[component]
pub fn IdeaEditor(id: Ulid) -> Element {
    let mut state = use_context::<Signal<AppState>>();

    let (title, body, categories_selected, requires_selected, all_categories, all_ideas) = {
        let s = state.read();
        let Some(idea) = s.idea(id) else {
            return rsx! { div { class: "empty", "アイデアが見つかりません" } };
        };
        (
            idea.meta.title.clone(),
            idea.body.clone(),
            idea.meta.categories.clone(),
            idea.meta.requires.clone(),
            s.categories.iter().map(|c| (c.meta.id, c.meta.name.clone())).collect::<Vec<_>>(),
            s.ideas.iter()
                .filter(|i| i.meta.id != id)
                .map(|i| (i.meta.id, i.meta.title.clone()))
                .collect::<Vec<_>>(),
        )
    };

    rsx! {
        div { class: "row",
            label { "タイトル" }
            input { r#type: "text", initial_value: "{title}",
                oninput: move |e| {
                    let v = e.value();
                    if let Some(i) = state.write().idea_mut(id) { i.meta.title = v; }
                },
                onblur: move |_| save_idea(state, id),
            }
            button { class: "danger",
                onclick: move |_| {
                    let _ = state.write().delete_idea(id);
                    state.write().selection = Selection::None;
                },
                "削除"
            }
        }
        div { class: "row",
            label { "カテゴリ" }
            div { class: "tags-input",
                for (cid, name) in all_categories.iter().cloned() {
                    {
                        let on = categories_selected.contains(&cid);
                        rsx! {
                            span {
                                key: "{cid}",
                                class: "tag",
                                style: if on { "background: #4a6cf7; color: #fff;" } else { "" },
                                onclick: move |_| {
                                    {
                                        let mut g = state.write();
                                        if let Some(i) = g.idea_mut(id) {
                                            if i.meta.categories.contains(&cid) {
                                                i.meta.categories.retain(|x| *x != cid);
                                            } else {
                                                i.meta.categories.push(cid);
                                            }
                                        }
                                    }
                                    save_idea(state, id);
                                },
                                "{name}"
                            }
                        }
                    }
                }
                if all_categories.is_empty() {
                    span { class: "sub", "カテゴリ未作成" }
                }
            }
        }
        div { class: "row",
            label { "依存 (requires)" }
            div { class: "tags-input",
                for (iid, ititle) in all_ideas.iter().cloned() {
                    {
                        let on = requires_selected.contains(&iid);
                        rsx! {
                            span {
                                key: "{iid}",
                                class: "tag",
                                style: if on { "background: #4a6cf7; color: #fff;" } else { "" },
                                onclick: move |_| {
                                    {
                                        let mut g = state.write();
                                        if let Some(i) = g.idea_mut(id) {
                                            if i.meta.requires.contains(&iid) {
                                                i.meta.requires.retain(|x| *x != iid);
                                            } else {
                                                i.meta.requires.push(iid);
                                            }
                                        }
                                    }
                                    save_idea(state, id);
                                },
                                "{ititle}"
                            }
                        }
                    }
                }
                if all_ideas.is_empty() {
                    span { class: "sub", "他のアイデアなし" }
                }
            }
        }
        textarea {
            style: "flex: 1; min-height: 300px;",
            initial_value: "{body}",
            oninput: move |e| {
                let v = e.value();
                if let Some(i) = state.write().idea_mut(id) { i.body = v; }
            },
            onblur: move |_| save_idea(state, id),
        }
    }
}

#[component]
pub fn CategoryEditor(id: Ulid) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let (name, body) = {
        let s = state.read();
        let Some(c) = s.category(id) else {
            return rsx! { div { class: "empty", "カテゴリが見つかりません" } };
        };
        (c.meta.name.clone(), c.body.clone())
    };
    rsx! {
        div { class: "row",
            label { "名前" }
            input { r#type: "text", initial_value: "{name}",
                oninput: move |e| {
                    let v = e.value();
                    if let Some(c) = state.write().category_mut(id) { c.meta.name = v; }
                },
                onblur: move |_| save_category(state, id),
            }
            button { class: "danger",
                onclick: move |_| {
                    let _ = state.write().delete_category(id);
                    state.write().selection = Selection::None;
                },
                "削除"
            }
        }
        textarea {
            style: "flex: 1; min-height: 300px;",
            initial_value: "{body}",
            oninput: move |e| {
                let v = e.value();
                if let Some(c) = state.write().category_mut(id) { c.body = v; }
            },
            onblur: move |_| save_category(state, id),
        }
    }
}

const STORY_BODY_DOM_ID: &str = "lm-story-body";

fn push_body_to_dom(text: &str) {
    let json = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into());
    let js = format!(
        "(()=>{{const ta=document.getElementById('{STORY_BODY_DOM_ID}');if(!ta)return;ta.value={json};ta.scrollTop=ta.scrollHeight;ta.selectionStart=ta.selectionEnd=ta.value.length;}})();"
    );
    let _ = dioxus::document::eval(&js);
}

#[derive(Debug, Clone)]
struct Proposal {
    id: u64,
    raw: String,
    pending: bool,
    error: Option<String>,
}

fn find_idx(props: &[Proposal], id: u64) -> Option<usize> {
    props.iter().position(|p| p.id == id)
}

#[derive(Debug, Clone)]
struct FillProposal {
    id: u64,
    /// `(hint, generated_text)` per slot, in slot order. Pending slots are empty.
    raw: String,
    pending: bool,
    error: Option<String>,
    /// Snapshot of slot hints captured at request time, so display stays stable
    /// even if the user edits the body while generation is in flight.
    hints: Vec<String>,
}

fn find_fill_idx(props: &[FillProposal], id: u64) -> Option<usize> {
    props.iter().position(|p| p.id == id)
}

#[component]
pub fn StoryEditor(id: Ulid) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut count = use_signal(|| 3u32);
    let mut proposals = use_signal::<Vec<Proposal>>(Vec::new);
    let mut next_proposal_id = use_signal(|| 1u64);
    let cancel_flag = use_hook(|| Arc::new(AtomicBool::new(false)));
    let mut generating = use_signal(|| false);
    let mut auto_running = use_signal(|| false);
    let mut auto_progress = use_signal(|| (0u32, 0u32));
    let mut auto_live = use_signal(String::new);
    let mut compacting = use_signal(|| false);
    let mut compact_live = use_signal(String::new);
    let mut compact_error = use_signal::<Option<String>>(|| None);
    let mut fill_proposals = use_signal::<Vec<FillProposal>>(Vec::new);
    let mut fill_generating = use_signal(|| false);
    let mut fill_error = use_signal::<Option<String>>(|| None);

    let disable_thinking = state.read().llm().disable_thinking;
    let (title, body, nsfw, all_ideas_with_status) = {
        let s = state.read();
        let Some(story) = s.story(id) else {
            return rsx! { div { class: "empty", "ストーリーが見つかりません" } };
        };
        let auto = resolve_active_ideas(&s.ideas, &story.meta.active_ideas);
        let auto_ids: std::collections::HashSet<Ulid> = auto.iter().map(|i| i.meta.id).collect();
        let manual: std::collections::HashSet<Ulid> = story.meta.active_ideas.iter().copied().collect();
        let chips: Vec<(Ulid, String, bool, bool)> = s.ideas.iter().map(|i| {
            let on = manual.contains(&i.meta.id);
            let auto_only = auto_ids.contains(&i.meta.id) && !on;
            (i.meta.id, i.meta.title.clone(), on, auto_only)
        }).collect();
        (
            story.meta.title.clone(),
            story.body.clone(),
            story.meta.nsfw,
            chips,
        )
    };

    let llm_cfg = state.read().llm().clone();
    let prompts_dir = state
        .read()
        .library
        .as_ref()
        .map(|l| l.prompts_dir())
        .unwrap_or_default();

    let on_generate = {
        let cancel_flag = cancel_flag.clone();
        let llm_cfg = llm_cfg.clone();
        let prompts_dir = prompts_dir.clone();
        move |_| {
            let n = *count.read();
            if n == 0 || *generating.read() { return; }
            cancel_flag.store(false, Ordering::SeqCst);
            generating.set(true);

            let snapshot_story = state.read().story(id).cloned();
            let snapshot_ideas = state.read().ideas.clone();
            let llm_cfg = llm_cfg.clone();
            let prompts_dir = prompts_dir.clone();
            let cancel_flag = cancel_flag.clone();

            spawn(async move {
                let Some(story) = snapshot_story else { generating.set(false); return; };
                let client = LlmClient::new(llm_cfg);
                for _ in 0..n {
                    if cancel_flag.load(Ordering::SeqCst) { break; }
                    let pid = {
                        let id = *next_proposal_id.read();
                        next_proposal_id.set(id + 1);
                        id
                    };
                    proposals.write().push(Proposal {
                        id: pid,
                        raw: String::new(),
                        pending: true,
                        error: None,
                    });
                    let on_delta = move |chunk: &str| {
                        let mut p = proposals.write();
                        if let Some(idx) = find_idx(&p, pid) {
                            p[idx].raw.push_str(chunk);
                        }
                    };
                    let result = client
                        .stream_continuation(
                            &prompts_dir,
                            &story,
                            &snapshot_ideas,
                            cancel_flag.clone(),
                            on_delta,
                        )
                        .await;
                    if cancel_flag.load(Ordering::SeqCst) {
                        let mut p = proposals.write();
                        if let Some(idx) = find_idx(&p, pid) { p.remove(idx); }
                        break;
                    }
                    let mut p = proposals.write();
                    if let Some(idx) = find_idx(&p, pid) {
                        match result {
                            Ok(raw) => p[idx] = Proposal { id: pid, raw, pending: false, error: None },
                            Err(e) => p[idx] = Proposal { id: pid, raw: String::new(), pending: false, error: Some(format!("{e}")) },
                        }
                    }
                }
                generating.set(false);
            });
        }
    };

    let on_auto = {
        let cancel_flag = cancel_flag.clone();
        let llm_cfg = llm_cfg.clone();
        let prompts_dir = prompts_dir.clone();
        move |_| {
            let n = *count.read();
            if n == 0 || *generating.read() || *auto_running.read() { return; }
            cancel_flag.store(false, Ordering::SeqCst);
            auto_running.set(true);
            auto_progress.set((0, n));
            auto_live.set(String::new());

            let snapshot_ideas = state.read().ideas.clone();
            let llm_cfg = llm_cfg.clone();
            let prompts_dir = prompts_dir.clone();
            let cancel_flag = cancel_flag.clone();

            spawn(async move {
                let client = LlmClient::new(llm_cfg);
                for i in 0..n {
                    if cancel_flag.load(Ordering::SeqCst) { break; }
                    auto_progress.set((i, n));
                    auto_live.set(String::new());

                    let Some(story) = state.read().story(id).cloned() else { break; };
                    let on_delta = move |chunk: &str| {
                        let mut s = auto_live.write();
                        s.push_str(chunk);
                    };
                    let result = client
                        .stream_continuation(
                            &prompts_dir,
                            &story,
                            &snapshot_ideas,
                            cancel_flag.clone(),
                            on_delta,
                        )
                        .await;
                    if cancel_flag.load(Ordering::SeqCst) { break; }
                    match result {
                        Ok(raw) => {
                            let visible = strip_author_notes(&visible_text(&raw).0).trim().to_string();
                            if visible.is_empty() {
                                tracing::warn!("auto mode: empty visible content, stopping");
                                break;
                            }
                            let new_body = {
                                let mut g = state.write();
                                if let Some(s) = g.story_mut(id) {
                                    s.append_body(&visible);
                                    s.body.clone()
                                } else { break; }
                            };
                            save_story(state, id);
                            push_body_to_dom(&new_body);
                        }
                        Err(e) => {
                            tracing::error!(?e, "auto mode generation failed");
                            break;
                        }
                    }
                }
                auto_progress.set((n, n));
                auto_live.set(String::new());
                auto_running.set(false);
            });
        }
    };

    let on_compact = {
        let cancel_flag = cancel_flag.clone();
        let llm_cfg = llm_cfg.clone();
        let prompts_dir = prompts_dir.clone();
        move |_| {
            if *generating.read() || *auto_running.read() || *compacting.read() { return; }
            cancel_flag.store(false, Ordering::SeqCst);
            compacting.set(true);
            compact_error.set(None);
            compact_live.set(String::new());

            let snapshot_story = state.read().story(id).cloned();
            let snapshot_ideas = state.read().ideas.clone();
            let llm_cfg = llm_cfg.clone();
            let prompts_dir = prompts_dir.clone();
            let cancel_flag = cancel_flag.clone();

            spawn(async move {
                let Some(story) = snapshot_story else { compacting.set(false); return; };
                let client = LlmClient::new(llm_cfg);
                let on_delta = move |chunk: &str| {
                    compact_live.write().push_str(chunk);
                };
                let result = client
                    .compact_body(&prompts_dir, &story, &snapshot_ideas, cancel_flag.clone(), on_delta)
                    .await;
                match result {
                    Ok(new_body) => {
                        {
                            let mut g = state.write();
                            if let Some(s) = g.story_mut(id) {
                                s.replace_body(new_body.clone());
                            }
                        }
                        save_story(state, id);
                        push_body_to_dom(&new_body);
                    }
                    Err(e) => {
                        compact_error.set(Some(format!("{e:#}")));
                    }
                }
                compact_live.set(String::new());
                compacting.set(false);
            });
        }
    };

    let on_fill = {
        let cancel_flag = cancel_flag.clone();
        let llm_cfg = llm_cfg.clone();
        let prompts_dir = prompts_dir.clone();
        move |_| {
            let n = *count.read();
            if n == 0 || *generating.read() || *auto_running.read() || *compacting.read() || *fill_generating.read() {
                return;
            }
            let snapshot_story = state.read().story(id).cloned();
            let snapshot_ideas = state.read().ideas.clone();
            let Some(story) = snapshot_story else { return; };
            let slots = extract_fills(&story.body);
            if slots.is_empty() {
                fill_error.set(Some("本文に `<!-- FILL: ... -->` がありません。「FILL を挿入」で穴を作ってください。".to_string()));
                return;
            }
            cancel_flag.store(false, Ordering::SeqCst);
            fill_generating.set(true);
            fill_error.set(None);
            let hints: Vec<String> = slots.iter().map(|s| s.hint.clone()).collect();
            let llm_cfg = llm_cfg.clone();
            let prompts_dir = prompts_dir.clone();
            let cancel_flag = cancel_flag.clone();
            spawn(async move {
                let client = LlmClient::new(llm_cfg);
                for _ in 0..n {
                    if cancel_flag.load(Ordering::SeqCst) { break; }
                    let pid = {
                        let id = *next_proposal_id.read();
                        next_proposal_id.set(id + 1);
                        id
                    };
                    fill_proposals.write().push(FillProposal {
                        id: pid,
                        raw: String::new(),
                        pending: true,
                        error: None,
                        hints: hints.clone(),
                    });
                    let on_delta = move |chunk: &str| {
                        let mut p = fill_proposals.write();
                        if let Some(idx) = find_fill_idx(&p, pid) {
                            p[idx].raw.push_str(chunk);
                        }
                    };
                    let result = client
                        .stream_fill(
                            &prompts_dir,
                            &story,
                            &snapshot_ideas,
                            &slots,
                            cancel_flag.clone(),
                            on_delta,
                        )
                        .await;
                    if cancel_flag.load(Ordering::SeqCst) {
                        let mut p = fill_proposals.write();
                        if let Some(idx) = find_fill_idx(&p, pid) { p.remove(idx); }
                        break;
                    }
                    let mut p = fill_proposals.write();
                    if let Some(idx) = find_fill_idx(&p, pid) {
                        match result {
                            Ok(raw) => {
                                p[idx].raw = raw;
                                p[idx].pending = false;
                            }
                            Err(e) => {
                                p[idx].pending = false;
                                p[idx].error = Some(format!("{e:#}"));
                            }
                        }
                    }
                }
                fill_generating.set(false);
            });
        }
    };

    let on_cancel = {
        let cancel_flag = cancel_flag.clone();
        move |_| {
            cancel_flag.store(true, Ordering::SeqCst);
        }
    };

    rsx! {
        div { class: "story-layout",
            // meta row
            div { class: "story-meta",
                input { r#type: "text", initial_value: "{title}",
                    oninput: move |e| {
                        let v = e.value();
                        if let Some(s) = state.write().story_mut(id) { s.meta.title = v; }
                    },
                    onblur: move |_| save_story(state, id),
                }
                label { style: "display: flex; gap: 4px; align-items: center;",
                    input { r#type: "checkbox", checked: nsfw,
                        oninput: move |e| {
                            let v = e.value() == "true";
                            if let Some(s) = state.write().story_mut(id) { s.meta.nsfw = v; }
                            save_story(state, id);
                        },
                    }
                    "NSFW"
                }
                label {
                    style: "display: flex; gap: 4px; align-items: center;",
                    title: "reasoning_effort=none を送信。Qwen3 等の reasoning モデルで思考トークンを抑止",
                    input { r#type: "checkbox", checked: disable_thinking,
                        oninput: move |e| {
                            let v = e.value() == "true";
                            state.write().config.llm.disable_thinking = v;
                            if let Err(e) = state.read().save_config() {
                                tracing::error!(?e, "save config");
                            }
                        },
                    }
                    "Thinking 無効"
                }
                button { class: "danger",
                    onclick: move |_| {
                        let _ = state.write().delete_story(id);
                        state.write().selection = Selection::None;
                    },
                    "削除"
                }
            }

            // active ideas
            div { class: "idea-toggles",
                for (iid, ititle, on, auto_only) in all_ideas_with_status {
                    {
                        let class = if auto_only { "chip auto" } else if on { "chip on" } else { "chip" };
                        rsx! {
                            span {
                                key: "{iid}",
                                class: "{class}",
                                title: if auto_only { "依存により自動有効" } else { "" },
                                onclick: move |_| {
                                    if auto_only { return; }
                                    if let Some(s) = state.write().story_mut(id) {
                                        if let Some(pos) = s.meta.active_ideas.iter().position(|x| *x == iid) {
                                            s.meta.active_ideas.remove(pos);
                                        } else {
                                            s.meta.active_ideas.push(iid);
                                        }
                                    }
                                    save_story(state, id);
                                },
                                "{ititle}"
                            }
                        }
                    }
                }
            }

            // body
            div { class: "story-body",
                textarea {
                    id: "{STORY_BODY_DOM_ID}",
                    initial_value: "{body}",
                    oninput: move |e| {
                        let v = e.value();
                        if let Some(s) = state.write().story_mut(id) { s.body = v; }
                    },
                    onblur: move |_| save_story(state, id),
                }
            }

            // generation panel
            div { class: "gen-panel",
                div { class: "gen-controls",
                    button {
                        class: "primary",
                        disabled: *generating.read() || *auto_running.read(),
                        onclick: on_generate,
                        title: "案を N 個生成して提示。採用したものだけ本文に追加",
                        if *generating.read() { "生成中…" } else { "続きを生成 (案出し)" }
                    }
                    button {
                        disabled: *generating.read() || *auto_running.read() || *compacting.read(),
                        onclick: on_auto,
                        title: "N 回連続で生成 → 自動で本文に追加。ボツ案なし",
                        {
                            let (done, total) = *auto_progress.read();
                            if *auto_running.read() {
                                rsx!{ "自動連投中… {done}/{total}" }
                            } else {
                                rsx!{ "自動連投" }
                            }
                        }
                    }
                    button {
                        disabled: *generating.read() || *auto_running.read() || *compacting.read(),
                        onclick: on_compact,
                        title: "本文の前半を要約に置き換えてコンテキスト消費を抑える。直近1500文字程度は残る",
                        if *compacting.read() { "圧縮中…" } else { "圧縮" }
                    }
                    label { "回数:" }
                    input { r#type: "number", min: "1", max: "20",
                        value: "{count}",
                        oninput: move |e| {
                            if let Ok(n) = e.value().parse::<u32>() { count.set(n.max(1).min(20)); }
                        },
                    }
                    button {
                        disabled: *generating.read() || *auto_running.read() || *compacting.read() || *fill_generating.read(),
                        onclick: on_fill,
                        title: "本文中の `<!-- FILL: ... -->` 全部を1回のAPIで埋める。指定回数ぶん独立した案を生成",
                        if *fill_generating.read() { "穴埋め中…" } else { "穴埋め生成" }
                    }
                    if *generating.read() || *auto_running.read() || *compacting.read() || *fill_generating.read() {
                        button { class: "danger", onclick: on_cancel, "キャンセル" }
                    }
                    if !proposals.read().is_empty() {
                        button {
                            onclick: move |_| proposals.set(Vec::new()),
                            "すべて破棄"
                        }
                    }
                    if !fill_proposals.read().is_empty() {
                        button {
                            onclick: move |_| fill_proposals.set(Vec::new()),
                            "穴埋め案を全破棄"
                        }
                    }
                    button {
                        title: "AIへの指示を本文末尾にHTMLコメントとして挿入。読者には見えず、生成時に指示として読まれる",
                        onclick: move |_| {
                            let new_body = {
                                let mut g = state.write();
                                let Some(s) = g.story_mut(id) else { return; };
                                if !s.body.is_empty() && !s.body.ends_with('\n') {
                                    s.body.push('\n');
                                }
                                s.body.push_str("<!-- NOTE: ここに指示を書く -->\n");
                                s.body.clone()
                            };
                            save_story(state, id);
                            push_body_to_dom(&new_body);
                        },
                        "著者注を挿入"
                    }
                    button {
                        title: "本文末尾にプレースホルダー `<!-- FILL: ヒント -->` を挿入。AIに穴埋めしてもらう箇所",
                        onclick: move |_| {
                            let new_body = {
                                let mut g = state.write();
                                let Some(s) = g.story_mut(id) else { return; };
                                if !s.body.is_empty() && !s.body.ends_with('\n') {
                                    s.body.push('\n');
                                }
                                s.body.push_str("<!-- FILL: ここを埋める -->\n");
                                s.body.clone()
                            };
                            save_story(state, id);
                            push_body_to_dom(&new_body);
                        },
                        "FILL を挿入"
                    }
                    span { style: "margin-left:auto; color:#8a8f99;",
                        "案: {proposals.read().len()} / 穴埋め案: {fill_proposals.read().len()}"
                    }
                }
                if let Some(err) = fill_error.read().clone() {
                    div { style: "background:#3a1f22; border:1px solid #6b3030; border-radius:4px; padding:10px; color:#ffb0b0;",
                        "穴埋め失敗: {err}"
                        button { style: "margin-left: 8px;",
                            onclick: move |_| fill_error.set(None),
                            "閉じる"
                        }
                    }
                }
                if let Some(err) = compact_error.read().clone() {
                    div { style: "background:#3a1f22; border:1px solid #6b3030; border-radius:4px; padding:10px; color:#ffb0b0;",
                        "圧縮失敗: {err}"
                        button { style: "margin-left: 8px;",
                            onclick: move |_| compact_error.set(None),
                            "閉じる"
                        }
                    }
                }
                if *compacting.read() {
                    {
                        let live = compact_live.read().clone();
                        let (visible, in_think) = visible_text(&live);
                        let visible = visible.trim_start().to_string();
                        rsx! {
                            div { class: "proposal pending",
                                div { style: "color:#9aa0aa; font-size: 11px; margin-bottom: 4px;",
                                    "圧縮中: 前半を要約に置換します"
                                }
                                div { class: "text",
                                    if visible.is_empty() {
                                        span { style: "color:#8a8f99;",
                                            if in_think { "考え中…" } else { "要約開始待ち…" }
                                        }
                                    } else {
                                        "{visible}"
                                        span { style: "color:#8a8f99;",
                                            if in_think { " 〔思考中〕" } else { " ▍" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if *auto_running.read() {
                    {
                        let live = auto_live.read().clone();
                        let (visible, in_think) = visible_text(&live);
                        let visible = visible.trim_start().to_string();
                        rsx! {
                            div { class: "proposal pending",
                                div { class: "text",
                                    if visible.is_empty() {
                                        span { style: "color:#8a8f99;",
                                            if in_think { "考え中…" } else { "生成開始待ち…" }
                                        }
                                    } else {
                                        "{visible}"
                                        span { style: "color:#8a8f99;",
                                            if in_think { " 〔思考中〕" } else { " ▍" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                for prop in proposals.read().iter().cloned() {
                    {
                        let pid = prop.id;
                        let (visible, in_think) = visible_text(&prop.raw);
                        let visible = strip_author_notes(&visible).trim_start().to_string();
                        rsx! {
                            div {
                                key: "{pid}",
                                class: if prop.pending { "proposal pending" } else { "proposal" },
                                if let Some(err) = prop.error.clone() {
                                    div { class: "text", style: "color:#ff8080;", "エラー: {err}" }
                                    div { class: "actions",
                                        button {
                                            onclick: move |_| {
                                                let mut p = proposals.write();
                                                if let Some(idx) = find_idx(&p, pid) { p.remove(idx); }
                                            },
                                            "閉じる"
                                        }
                                    }
                                } else {
                                    if visible.is_empty() && prop.pending {
                                        div { class: "text", style: "color:#8a8f99;",
                                            if in_think { "考え中…" } else { "生成開始待ち…" }
                                        }
                                    } else {
                                        div { class: "text",
                                            "{visible}"
                                            if prop.pending {
                                                span { style: "color:#8a8f99;",
                                                    if in_think { " 〔思考中〕" } else { " ▍" }
                                                }
                                            }
                                        }
                                    }
                                    div { class: "actions",
                                        button {
                                            disabled: prop.pending,
                                            onclick: move |_| {
                                                let mut p = proposals.write();
                                                if let Some(idx) = find_idx(&p, pid) { p.remove(idx); }
                                            },
                                            "破棄"
                                        }
                                        button {
                                            class: "primary",
                                            disabled: prop.pending || visible.is_empty(),
                                            onclick: {
                                                let text = visible.clone();
                                                move |_| {
                                                    let new_body = {
                                                        let mut g = state.write();
                                                        if let Some(s) = g.story_mut(id) {
                                                            s.append_body(&text);
                                                            s.body.clone()
                                                        } else {
                                                            return;
                                                        }
                                                    };
                                                    save_story(state, id);
                                                    push_body_to_dom(&new_body);
                                                    let mut p = proposals.write();
                                                    if let Some(idx) = find_idx(&p, pid) { p.remove(idx); }
                                                }
                                            },
                                            "採用"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                for prop in fill_proposals.read().iter().cloned() {
                    {
                        let pid = prop.id;
                        let (visible, in_think) = visible_text(&prop.raw);
                        let parsed = if prop.pending { Default::default() } else { parse_fill_response(&visible) };
                        rsx! {
                            div {
                                key: "fill-{pid}",
                                class: if prop.pending { "proposal pending" } else { "proposal" },
                                div { style: "color:#9aa0aa; font-size: 11px; margin-bottom: 4px;",
                                    "穴埋め案: {prop.hints.len()} スロット"
                                }
                                if let Some(err) = prop.error.clone() {
                                    div { class: "text", style: "color:#ff8080;", "エラー: {err}" }
                                    div { class: "actions",
                                        button {
                                            onclick: move |_| {
                                                let mut p = fill_proposals.write();
                                                if let Some(idx) = find_fill_idx(&p, pid) { p.remove(idx); }
                                            },
                                            "閉じる"
                                        }
                                    }
                                } else if prop.pending {
                                    div { class: "text", style: "color:#8a8f99;",
                                        if visible.trim().is_empty() {
                                            if in_think { "考え中…" } else { "生成開始待ち…" }
                                        } else {
                                            "ストリーミング中… ({visible.len()} 文字受信)"
                                            if in_think { " 〔思考中〕" }
                                        }
                                    }
                                } else {
                                    div { class: "text",
                                        for (i, hint) in prop.hints.iter().enumerate() {
                                            {
                                                let n = i + 1;
                                                let filled = parsed.get(&n).cloned().unwrap_or_default();
                                                let label = if hint.is_empty() {
                                                    format!("#{n}")
                                                } else {
                                                    format!("#{n}: {hint}")
                                                };
                                                let is_missing = filled.trim().is_empty();
                                                rsx! {
                                                    div {
                                                        key: "{n}",
                                                        style: "margin-bottom: 8px; padding-left: 8px; border-left: 2px solid #4a6cf7;",
                                                        div { style: "color:#9aa0aa; font-size: 11px; margin-bottom: 2px;", "{label}" }
                                                        if is_missing {
                                                            div { style: "color:#ff8080; font-size: 12px;", "(モデルが #{n} を返さなかった — 採用すると元の FILL マーカーが残ります)" }
                                                        } else {
                                                            div { "{filled}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    div { class: "actions",
                                        button {
                                            onclick: move |_| {
                                                let mut p = fill_proposals.write();
                                                if let Some(idx) = find_fill_idx(&p, pid) { p.remove(idx); }
                                            },
                                            "破棄"
                                        }
                                        button {
                                            class: "primary",
                                            disabled: parsed.is_empty(),
                                            onclick: {
                                                let parsed = parsed.clone();
                                                move |_| {
                                                    let new_body = {
                                                        let mut g = state.write();
                                                        let Some(s) = g.story_mut(id) else { return; };
                                                        let slots = extract_fills(&s.body);
                                                        let new = apply_fills(&s.body, &slots, &parsed);
                                                        s.replace_body(new);
                                                        s.body.clone()
                                                    };
                                                    save_story(state, id);
                                                    push_body_to_dom(&new_body);
                                                    let mut p = fill_proposals.write();
                                                    if let Some(idx) = find_fill_idx(&p, pid) { p.remove(idx); }
                                                }
                                            },
                                            "採用"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
