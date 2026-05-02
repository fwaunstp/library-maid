use super::state::AppState;
use crate::llm::LlmConfig;
use dioxus::prelude::*;

#[derive(Debug, Clone)]
enum ProbeState {
    Idle,
    Running,
    Ok(Vec<String>),
    Err(String),
}

#[component]
pub fn LlmSettings() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut probe = use_signal(|| ProbeState::Idle);

    let snapshot = state.read().llm().clone();

    let save = move || {
        if let Err(e) = state.read().save_config() {
            tracing::error!(?e, "save config");
        }
    };

    rsx! {
        h2 { "LLM 接続" }
        p { class: "sub", style: "color:#8a8f99;",
            "llama.cpp の OpenAI 互換 server (例: ", code { "llama-server -m model.gguf --port 8080" }, ") に接続します。"
        }

        Field {
            label: "API Base URL",
            hint: "例: http://127.0.0.1:8080/v1",
            value: snapshot.api_base.clone(),
            on_change: move |v: String| {
                state.write().config.llm.api_base = v;
            },
            on_commit: move |_| save(),
        }
        Field {
            label: "API Key",
            hint: "llama.cpp は無視するが、空欄不可",
            value: snapshot.api_key.clone(),
            on_change: move |v: String| {
                state.write().config.llm.api_key = v;
            },
            on_commit: move |_| save(),
        }
        Field {
            label: "モデル名",
            hint: "/v1/models で返ってくる id。llama.cpp は通常無視するので任意で可",
            value: snapshot.model.clone(),
            on_change: move |v: String| {
                state.write().config.llm.model = v;
            },
            on_commit: move |_| save(),
        }

        h3 { style: "margin-top: 16px;", "生成パラメータ" }
        NumField {
            label: "max_tokens",
            value: snapshot.max_tokens as f64,
            step: 256.0,
            integer: true,
            hint: "1案あたりの最大出力トークン数。reasoning モデル (Qwen3 等) は思考に使うので 4096〜8192 を推奨。長すぎると 1 案あたり時間がかかる",
            on_change: move |v: f64| {
                state.write().config.llm.max_tokens = v.max(1.0) as u32;
            },
            on_commit: move |_| save(),
        }
        NumField {
            label: "temperature",
            value: snapshot.temperature as f64,
            step: 0.05,
            integer: false,
            hint: "高いほど多様、低いほど決定的。NSFW 創作では 0.8〜1.0 程度が無難",
            on_change: move |v: f64| {
                state.write().config.llm.temperature = v as f32;
            },
            on_commit: move |_| save(),
        }
        NumField {
            label: "top_p",
            value: snapshot.top_p as f64,
            step: 0.05,
            integer: false,
            hint: "上位 p% の累積確率トークンから抽出。0.9〜0.95 が一般的",
            on_change: move |v: f64| {
                state.write().config.llm.top_p = v as f32;
            },
            on_commit: move |_| save(),
        }

        div { class: "row", style: "margin-top: 16px; gap: 12px;",
            button {
                class: "primary",
                disabled: matches!(*probe.read(), ProbeState::Running),
                onclick: move |_| {
                    let cfg: LlmConfig = state.read().llm().clone();
                    probe.set(ProbeState::Running);
                    spawn(async move {
                        match cfg.probe().await {
                            Ok(ids) => probe.set(ProbeState::Ok(ids)),
                            Err(e) => probe.set(ProbeState::Err(format!("{e:#}"))),
                        }
                    });
                },
                if matches!(*probe.read(), ProbeState::Running) { "確認中…" } else { "接続テスト" }
            }
            button {
                onclick: move |_| {
                    state.write().config.llm = LlmConfig::default();
                    save();
                    probe.set(ProbeState::Idle);
                },
                "デフォルトに戻す"
            }
        }

        {
            match probe.read().clone() {
                ProbeState::Idle => rsx!{},
                ProbeState::Running => rsx!{ div { class: "sub", style: "color:#8a8f99;", "問い合わせ中…" } },
                ProbeState::Ok(ids) => rsx!{
                    div { style: "background:#1f3022; border:1px solid #2e5a3c; border-radius:4px; padding:10px;",
                        div { style: "color:#9ee6b4;", "接続OK" }
                        if ids.is_empty() {
                            div { class: "sub", "(モデル一覧は空でした)" }
                        } else {
                            div { class: "sub", "利用可能モデル:" }
                            ul { style: "margin: 4px 0 0 16px; padding: 0;",
                                for id in ids {
                                    li { key: "{id}",
                                        code { "{id}" }
                                        " "
                                        button {
                                            style: "padding: 0 6px; font-size: 11px;",
                                            onclick: {
                                                let id = id.clone();
                                                move |_| {
                                                    state.write().config.llm.model = id.clone();
                                                    save();
                                                }
                                            },
                                            "適用"
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                ProbeState::Err(e) => rsx!{
                    div { style: "background:#3a1f22; border:1px solid #6b3030; border-radius:4px; padding:10px; color:#ffb0b0;",
                        "接続失敗: {e}"
                    }
                },
            }
        }
    }
}

#[component]
fn Field(
    label: &'static str,
    hint: &'static str,
    value: String,
    on_change: EventHandler<String>,
    on_commit: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "row", style: "align-items: flex-start;",
            label { style: "min-width: 130px; padding-top: 6px;", "{label}" }
            div { style: "flex: 1; display: flex; flex-direction: column; gap: 2px;",
                input { r#type: "text", initial_value: "{value}",
                    oninput: move |e| on_change.call(e.value()),
                    onblur: move |_| on_commit.call(()),
                }
                div { class: "sub", style: "color:#6b6f78; font-size: 11px;", "{hint}" }
            }
        }
    }
}

#[component]
fn NumField(
    label: &'static str,
    value: f64,
    step: f64,
    integer: bool,
    hint: &'static str,
    on_change: EventHandler<f64>,
    on_commit: EventHandler<()>,
) -> Element {
    let displayed = if integer {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    };
    rsx! {
        div { class: "row", style: "align-items: flex-start;",
            label { style: "min-width: 130px; padding-top: 6px;", "{label}" }
            div { style: "flex: 1; display: flex; flex-direction: column; gap: 2px;",
                input { r#type: "number", value: "{displayed}", step: "{step}",
                    style: "width: 140px;",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<f64>() { on_change.call(v); }
                    },
                    onblur: move |_| on_commit.call(()),
                }
                div { class: "sub", style: "color:#6b6f78; font-size: 11px;", "{hint}" }
            }
        }
    }
}
