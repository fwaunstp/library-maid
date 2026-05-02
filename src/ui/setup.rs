use super::AppState;
use crate::config;
use dioxus::prelude::*;

#[component]
pub fn SetupScreen() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut error = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "setup",
            h1 { "library-maid へようこそ" }
            p {
                "アイデアと小説の保存先フォルダを選択してください。"
                br {}
                "Markdown ファイル + YAML フロントマターで保存され、git 管理に向いた構成になります。"
            }
            button {
                class: "primary",
                onclick: move |_| {
                    error.set(None);
                    if let Some(dir) = config::pick_data_dir() {
                        if let Err(e) = state.write().set_data_dir(dir) {
                            error.set(Some(format!("設定の保存に失敗: {e}")));
                        }
                    }
                },
                "保存先フォルダを選ぶ…"
            }
            if let Some(err) = error() {
                p { style: "color: #ff8080;", "{err}" }
            }
        }
    }
}
