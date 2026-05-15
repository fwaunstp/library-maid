use super::state::{AppState, ProbeState, SettingsEvent};
use super::theme;
use crate::llm::LlmConfig;
use crate::tts::TtsConfig;

pub fn drain_settings_events(state: &mut AppState) {
    let mut events = Vec::new();
    while let Ok(ev) = state.settings.rx.try_recv() {
        events.push(ev);
    }
    for ev in events {
        match ev {
            SettingsEvent::ProbeOk(ids) => state.settings.probe = ProbeState::Ok(ids),
            SettingsEvent::ProbeErr(e) => state.settings.probe = ProbeState::Err(e),
        }
    }
}

pub fn llm_settings(state: &mut AppState, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("LLM 接続");
        ui.label(
            egui::RichText::new(
                "llama.cpp の OpenAI 互換 server (例: llama-server -m model.gguf --port 8080) に接続します。",
            )
            .color(theme::SUBTLE_TEXT),
        );
        ui.add_space(8.0);

        let mut commit = false;
        text_field(
            ui,
            "API Base URL",
            "例: http://127.0.0.1:8080/v1",
            &mut state.config.llm.api_base,
            &mut commit,
        );
        text_field(
            ui,
            "API Key",
            "llama.cpp は無視するが、空欄不可",
            &mut state.config.llm.api_key,
            &mut commit,
        );
        text_field(
            ui,
            "モデル名",
            "/v1/models で返ってくる id。llama.cpp は通常無視するので任意で可",
            &mut state.config.llm.model,
            &mut commit,
        );

        ui.add_space(8.0);
        ui.heading("生成パラメータ");

        u32_field(
            ui,
            "max_tokens",
            "1案あたりの最大出力トークン数。reasoning モデルは 4096〜8192 を推奨",
            &mut state.config.llm.max_tokens,
            256,
            &mut commit,
        );
        f32_field(
            ui,
            "temperature",
            "高いほど多様、低いほど決定的。NSFW 創作では 0.8〜1.0 程度が無難",
            &mut state.config.llm.temperature,
            0.05,
            &mut commit,
        );
        f32_field(
            ui,
            "top_p",
            "上位 p% の累積確率トークンから抽出。0.9〜0.95 が一般的",
            &mut state.config.llm.top_p,
            0.05,
            &mut commit,
        );

        if commit {
            if let Err(e) = state.save_config() {
                tracing::error!(?e, "save config");
            }
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let probing = matches!(state.settings.probe, ProbeState::Running);
            if theme::primary_button_enabled(
                ui,
                !probing,
                if probing { "確認中…" } else { "接続テスト" },
            )
            .clicked()
            {
                state.settings.probe = ProbeState::Running;
                let cfg: LlmConfig = state.config.llm.clone();
                let tx = state.settings.tx.clone();
                let ctx = state.egui_ctx.clone();
                state.rt.spawn(async move {
                    match cfg.probe().await {
                        Ok(ids) => {
                            let _ = tx.send(SettingsEvent::ProbeOk(ids));
                        }
                        Err(e) => {
                            let _ = tx.send(SettingsEvent::ProbeErr(format!("{e:#}")));
                        }
                    }
                    ctx.request_repaint();
                });
            }
            if ui.button("デフォルトに戻す").clicked() {
                state.config.llm = LlmConfig::default();
                if let Err(e) = state.save_config() {
                    tracing::error!(?e, "save config");
                }
                state.settings.probe = ProbeState::Idle;
            }
        });

        ui.add_space(8.0);
        let mut model_to_apply: Option<String> = None;
        match state.settings.probe.clone() {
            ProbeState::Idle => {}
            ProbeState::Running => {
                ui.label(egui::RichText::new("問い合わせ中…").color(theme::SUBTLE_TEXT));
            }
            ProbeState::Ok(ids) => {
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(0x1f, 0x30, 0x22))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(0x2e, 0x5a, 0x3c),
                    ))
                    .corner_radius(egui::CornerRadius::same(4))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("接続OK")
                                .color(egui::Color32::from_rgb(0x9e, 0xe6, 0xb4)),
                        );
                        if ids.is_empty() {
                            ui.label(
                                egui::RichText::new("(モデル一覧は空でした)")
                                    .color(theme::SUBTLE_TEXT),
                            );
                        } else {
                            ui.label("利用可能モデル:");
                            for id in &ids {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(id).monospace());
                                    if ui.small_button("適用").clicked() {
                                        model_to_apply = Some(id.clone());
                                    }
                                });
                            }
                        }
                    });
            }
            ProbeState::Err(e) => {
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(0x3a, 0x1f, 0x22))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(0x6b, 0x30, 0x30),
                    ))
                    .corner_radius(egui::CornerRadius::same(4))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("接続失敗: {e}"))
                                .color(theme::ERROR_TEXT),
                        );
                    });
            }
        }
        if let Some(m) = model_to_apply {
            state.config.llm.model = m;
            if let Err(e) = state.save_config() {
                tracing::error!(?e, "save config");
            }
        }
    });
}

pub fn tts_settings(state: &mut AppState, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("TTS 接続 (読み上げ)");
        ui.label(
            egui::RichText::new(
                "OpenAI 互換の /v1/audio/speech エンドポイントに接続します。\
                 既定は OpenAI 公式 (gpt-4o-mini-tts, voice=alloy)。\
                 互換サーバを使う場合は API Base URL を書き換えてください。",
            )
            .color(theme::SUBTLE_TEXT),
        );
        ui.add_space(8.0);

        let mut commit = false;
        text_field(
            ui,
            "API Base URL",
            "例: https://api.openai.com/v1",
            &mut state.config.tts.api_base,
            &mut commit,
        );
        text_field(
            ui,
            "API Key",
            "OpenAI の sk-... など。空欄では再生不可",
            &mut state.config.tts.api_key,
            &mut commit,
        );
        text_field(
            ui,
            "モデル",
            "OpenAI: tts-1 / tts-1-hd / gpt-4o-mini-tts",
            &mut state.config.tts.model,
            &mut commit,
        );
        text_field(
            ui,
            "Voice",
            "OpenAI: alloy / ash / ballad / coral / echo / fable / nova / onyx / sage / shimmer",
            &mut state.config.tts.voice,
            &mut commit,
        );
        f32_field(
            ui,
            "速度",
            "0.25 ~ 4.0。1.0 が等速",
            &mut state.config.tts.speed,
            0.05,
            &mut commit,
        );

        if commit {
            if let Err(e) = state.save_config() {
                tracing::error!(?e, "save config");
            }
        }

        ui.add_space(12.0);
        if ui.button("デフォルトに戻す").clicked() {
            state.config.tts = TtsConfig::default();
            if let Err(e) = state.save_config() {
                tracing::error!(?e, "save config");
            }
        }
    });
}

fn text_field(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    value: &mut String,
    commit: &mut bool,
) {
    ui.add_space(4.0);
    ui.label(label);
    let resp = ui.add(egui::TextEdit::singleline(value).desired_width(420.0));
    if resp.lost_focus() {
        *commit = true;
    }
    ui.label(egui::RichText::new(hint).small().color(theme::SUBTLE_TEXT));
}

fn u32_field(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    value: &mut u32,
    step: u32,
    commit: &mut bool,
) {
    ui.add_space(4.0);
    ui.label(label);
    let mut v = *value;
    let resp = ui.add(
        egui::DragValue::new(&mut v)
            .range(1..=u32::MAX)
            .speed(step as f32),
    );
    if resp.changed() {
        *value = v.max(1);
        *commit = true;
    }
    if resp.lost_focus() {
        *commit = true;
    }
    ui.label(egui::RichText::new(hint).small().color(theme::SUBTLE_TEXT));
}

fn f32_field(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    value: &mut f32,
    step: f32,
    commit: &mut bool,
) {
    ui.add_space(4.0);
    ui.label(label);
    let resp = ui.add(egui::DragValue::new(value).speed(step as f64));
    if resp.changed() {
        *commit = true;
    }
    if resp.lost_focus() {
        *commit = true;
    }
    ui.label(egui::RichText::new(hint).small().color(theme::SUBTLE_TEXT));
}
