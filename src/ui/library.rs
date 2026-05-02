use super::editors;
use super::settings;
use super::state::{AppState, Selection, SettingsSection, Tab};
use dioxus::prelude::*;

#[component]
pub fn LibraryScreen() -> Element {
    let state = use_context::<Signal<AppState>>();
    let tab = state.read().tab;
    let data_dir = state
        .read()
        .config
        .data_dir
        .clone()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    rsx! {
        div { class: "library",
            // sidebar tabs
            div { class: "tabs",
                TabButton { tab: Tab::Ideas, label: "アイデア" }
                TabButton { tab: Tab::Categories, label: "カテゴリ" }
                TabButton { tab: Tab::Stories, label: "ストーリー" }
                TabButton { tab: Tab::Settings, label: "設定" }
                div { class: "footer", "data: {data_dir}" }
            }

            // middle list
            div { class: "list",
                match tab {
                    Tab::Ideas => rsx!{ IdeaList {} },
                    Tab::Categories => rsx!{ CategoryList {} },
                    Tab::Stories => rsx!{ StoryList {} },
                    Tab::Settings => rsx!{ SettingsList {} },
                }
            }

            // right pane
            div { class: "editor",
                {
                    let selection = state.read().selection.clone();
                    match selection {
                        Selection::Idea(id) => rsx!{ editors::IdeaEditor { key: "idea-{id}", id: id } },
                        Selection::Category(id) => rsx!{ editors::CategoryEditor { key: "category-{id}", id: id } },
                        Selection::Story(id) => rsx!{ editors::StoryEditor { key: "story-{id}", id: id } },
                        Selection::Settings(SettingsSection::Llm) => rsx!{ settings::LlmSettings {} },
                        Selection::None => rsx!{ div { class: "empty", "左のリストから項目を選ぶか、新規作成してください。" } },
                    }
                }
            }
        }
    }
}

#[component]
fn TabButton(tab: Tab, label: &'static str) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let active = state.read().tab == tab;
    rsx! {
        button {
            class: if active { "active" } else { "" },
            onclick: move |_| {
                let mut s = state.write();
                s.tab = tab;
                s.selection = Selection::None;
            },
            "{label}"
        }
    }
}

#[component]
fn SettingsList() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let selected = matches!(state.read().selection, Selection::Settings(SettingsSection::Llm));
    rsx! {
        div {
            class: if selected { "list-item selected" } else { "list-item" },
            onclick: move |_| {
                state.write().selection = Selection::Settings(SettingsSection::Llm);
            },
            div { class: "title", "LLM 接続" }
            div { class: "sub", "llama.cpp サーバーのエンドポイント等" }
        }
    }
}

#[component]
fn IdeaList() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let category_lookup: std::collections::HashMap<_, _> = state
        .read()
        .categories
        .iter()
        .map(|c| (c.meta.id, c.meta.name.clone()))
        .collect();
    let items: Vec<_> = state
        .read()
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

    rsx! {
        div { class: "list-toolbar",
            button { class: "primary",
                onclick: move |_| {
                    let res = state.write().create_idea();
                    if let Ok(id) = res {
                        state.write().selection = Selection::Idea(id);
                    }
                },
                "+ 新規"
            }
        }
        for (id, title, sub) in items {
            ListItem {
                key: "{id}",
                title: title.clone(),
                sub: sub.clone(),
                selected: matches!(state.read().selection, Selection::Idea(s) if s == id),
                on_click: move |_| state.write().selection = Selection::Idea(id),
            }
        }
    }
}

#[component]
fn CategoryList() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let items: Vec<_> = state
        .read()
        .categories
        .iter()
        .map(|c| (c.meta.id, c.meta.name.clone()))
        .collect();
    rsx! {
        div { class: "list-toolbar",
            button { class: "primary",
                onclick: move |_| {
                    let res = state.write().create_category();
                    if let Ok(id) = res {
                        state.write().selection = Selection::Category(id);
                    }
                },
                "+ 新規"
            }
        }
        for (id, name) in items {
            ListItem {
                key: "{id}",
                title: name.clone(),
                sub: "".to_string(),
                selected: matches!(state.read().selection, Selection::Category(s) if s == id),
                on_click: move |_| state.write().selection = Selection::Category(id),
            }
        }
    }
}

#[component]
fn StoryList() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let items: Vec<_> = state
        .read()
        .stories
        .iter()
        .map(|s| {
            let lang = match s.meta.language {
                crate::data::story::Language::Ja => "JA",
                crate::data::story::Language::En => "EN",
            };
            let nsfw = if s.meta.nsfw { "NSFW" } else { "SFW" };
            (s.meta.id, s.meta.title.clone(), format!("{lang} · {nsfw}"))
        })
        .collect();
    rsx! {
        div { class: "list-toolbar",
            button { class: "primary",
                onclick: move |_| {
                    let res = state.write().create_story();
                    if let Ok(id) = res {
                        state.write().selection = Selection::Story(id);
                    }
                },
                "+ 新規"
            }
        }
        for (id, title, sub) in items {
            ListItem {
                key: "{id}",
                title: title.clone(),
                sub: sub.clone(),
                selected: matches!(state.read().selection, Selection::Story(s) if s == id),
                on_click: move |_| state.write().selection = Selection::Story(id),
            }
        }
    }
}

#[component]
fn ListItem(title: String, sub: String, selected: bool, on_click: EventHandler<MouseEvent>) -> Element {
    rsx! {
        div {
            class: if selected { "list-item selected" } else { "list-item" },
            onclick: move |e| on_click.call(e),
            div { class: "title", "{title}" }
            if !sub.is_empty() {
                div { class: "sub", "{sub}" }
            }
        }
    }
}
