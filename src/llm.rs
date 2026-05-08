use crate::data::{Idea, Story};
use crate::prompts::{self, PromptKey};
use anyhow::{Context, Result};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
        FinishReason,
        ReasoningEffort,
    },
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use ulid::Ulid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
    /// When true, send `reasoning_effort=none` to ask the server to skip thinking.
    /// llama.cpp maps this to `enable_thinking=false` for Qwen3 etc.
    pub disable_thinking: bool,
    /// Compaction: number of trailing characters of the body kept verbatim
    /// (the rest gets summarized). Snapped forward to the next paragraph break.
    pub compact_keep_recent_chars: usize,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_base: "http://127.0.0.1:8080/v1".into(),
            api_key: "sk-no-key-required".into(),
            model: "local".into(),
            temperature: 0.9,
            top_p: 0.95,
            max_tokens: 4096,
            disable_thinking: true,
            compact_keep_recent_chars: 1500,
        }
    }
}

impl LlmConfig {
    pub async fn probe(&self) -> Result<Vec<String>> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        let url = format!("{}/models", self.api_base.trim_end_matches('/'));
        let resp = client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?
            .error_for_status()
            .with_context(|| format!("non-2xx from {url}"))?;
        let json: serde_json::Value = resp.json().await.context("parse /models json")?;
        let mut ids = Vec::new();
        if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    ids.push(id.to_string());
                }
            }
        }
        Ok(ids)
    }
}

pub struct LlmClient {
    client: Client<OpenAIConfig>,
    cfg: LlmConfig,
}

impl LlmClient {
    pub fn new(cfg: LlmConfig) -> Self {
        let openai_cfg = OpenAIConfig::new()
            .with_api_base(cfg.api_base.clone())
            .with_api_key(cfg.api_key.clone());
        Self {
            client: Client::with_config(openai_cfg),
            cfg,
        }
    }

    /// Stream `count` independent candidate continuations of the draft in a
    /// single API call. The model returns `# #1\n…\n# #2\n…` blocks; the
    /// caller parses them with `parse_numbered_blocks`. Each candidate is an
    /// alternative for the same draft (not a chain).
    pub async fn stream_proposals<F: FnMut(&str) + Send>(
        &self,
        prompts_dir: &Path,
        story: &Story,
        ideas: &[Idea],
        count: u32,
        cancel: Arc<AtomicBool>,
        on_delta: F,
    ) -> Result<String> {
        let key = PromptKey { nsfw: story.meta.nsfw };
        let template = prompts::load_template(prompts_dir, key)?;
        self.stream_numbered(prompts_dir, story, ideas, &template, count, cancel, on_delta)
            .await
    }

    /// Stream `count` sequential continuations in a single API call (auto-continue
    /// mode). The model returns `# #1\n…\n# #2\n…` blocks where each block
    /// picks up where the previous one ended. The caller appends them in order
    /// to the draft.
    pub async fn stream_auto_batch<F: FnMut(&str) + Send>(
        &self,
        prompts_dir: &Path,
        story: &Story,
        ideas: &[Idea],
        count: u32,
        cancel: Arc<AtomicBool>,
        on_delta: F,
    ) -> Result<String> {
        let key = PromptKey { nsfw: story.meta.nsfw };
        let template = prompts::load_auto_template(prompts_dir, key)?;
        self.stream_numbered(prompts_dir, story, ideas, &template, count, cancel, on_delta)
            .await
    }

    /// Shared implementation for batched `# #N`-formatted output.
    async fn stream_numbered<F: FnMut(&str) + Send>(
        &self,
        _prompts_dir: &Path,
        story: &Story,
        ideas: &[Idea],
        template: &str,
        count: u32,
        cancel: Arc<AtomicBool>,
        mut on_delta: F,
    ) -> Result<String> {
        let count = count.max(1);
        let active = resolve_active_ideas(ideas, &story.meta.active_ideas);
        let ideas_block = format_ideas(&active);
        let body_block = if story.body.trim().is_empty() {
            placeholder_empty_body().to_string()
        } else {
            story.body.clone()
        };
        let count_str = count.to_string();
        let user_prompt = prompts::render(
            template,
            &[
                ("ideas", &ideas_block),
                ("body", &body_block),
                ("count", &count_str),
            ],
        );

        let mut builder = CreateChatCompletionRequestArgs::default();
        builder
            .model(&self.cfg.model)
            .temperature(self.cfg.temperature)
            .top_p(self.cfg.top_p)
            .max_tokens(self.cfg.max_tokens)
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(user_prompt)
                .build()?
                .into()]);
        if self.cfg.disable_thinking {
            builder.reasoning_effort(ReasoningEffort::None);
        }
        let request = builder.build()?;

        let mut stream = self
            .client
            .chat()
            .create_stream(request)
            .await
            .context("create_stream failed")?;

        let mut accumulated = String::new();
        let mut last_finish: Option<FinishReason> = None;

        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                return Ok(accumulated);
            }
            let chunk = chunk.context("stream chunk error")?;
            for choice in chunk.choices {
                if let Some(reason) = choice.finish_reason {
                    last_finish = Some(reason);
                }
                if let Some(content) = choice.delta.content {
                    if !content.is_empty() {
                        accumulated.push_str(&content);
                        on_delta(&content);
                    }
                }
            }
        }

        let visible = visible_text(&accumulated).0;
        if visible.trim().is_empty() {
            let hint = match last_finish {
                Some(FinishReason::Length) => {
                    "max_tokens に達した。reasoning モデル (Qwen3, DeepSeek 等) が thinking で枠を使い切った可能性。\n対処: 設定の max_tokens を増やす / llama-server に `--reasoning-format none` を付けて起動 / 非 reasoning モデルを使う"
                }
                Some(FinishReason::ContentFilter) => {
                    "content_filter で削除された (NSFW 拒否)。別モデルを検討してください"
                }
                _ => "プロンプト形式やテンプレートを確認してください",
            };
            return Err(anyhow::anyhow!(
                "empty visible content (finish_reason={last_finish:?})\n→ {hint}"
            ));
        }
        Ok(accumulated)
    }
}

impl LlmClient {
    /// Summarize the older portion of `body` and return a new body where
    /// that portion is replaced with the summary, separated by `〔ここまでの要約〕…〔要約ここまで〕`
    /// markers. The trailing `keep_recent_chars` characters (snapped forward
    /// to the next paragraph break) are preserved verbatim.
    pub async fn compact_body<F: FnMut(&str) + Send>(
        &self,
        prompts_dir: &Path,
        story: &Story,
        ideas: &[Idea],
        cancel: Arc<AtomicBool>,
        mut on_delta: F,
    ) -> Result<String> {
        let (to_summarize, kept_recent) =
            split_for_compaction(&story.body, self.cfg.compact_keep_recent_chars);
        if to_summarize.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "本文が短すぎて圧縮できません (要約対象が空)。compact_keep_recent_chars を下げてください。"
            ));
        }

        let template = prompts::load_compact_template(prompts_dir)?;
        let active = resolve_active_ideas(ideas, &story.meta.active_ideas);
        let ideas_block = format_ideas(&active);
        let user_prompt = prompts::render(
            &template,
            &[("ideas", &ideas_block), ("body", &to_summarize)],
        );

        let mut builder = CreateChatCompletionRequestArgs::default();
        builder
            .model(&self.cfg.model)
            .temperature(0.5)
            .top_p(0.9)
            .max_tokens(self.cfg.max_tokens)
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(user_prompt)
                .build()?
                .into()]);
        if self.cfg.disable_thinking {
            builder.reasoning_effort(ReasoningEffort::None);
        }
        let request = builder.build()?;

        let mut stream = self
            .client
            .chat()
            .create_stream(request)
            .await
            .context("compact create_stream failed")?;

        let mut accumulated = String::new();
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                return Err(anyhow::anyhow!("cancelled"));
            }
            let chunk = chunk.context("stream chunk error")?;
            for choice in chunk.choices {
                if let Some(content) = choice.delta.content {
                    if !content.is_empty() {
                        accumulated.push_str(&content);
                        on_delta(&content);
                    }
                }
            }
        }

        let summary = visible_text(&accumulated).0.trim().to_string();
        if summary.is_empty() {
            return Err(anyhow::anyhow!(
                "要約結果が空でした。max_tokens を増やすかモデル/設定を見直してください"
            ));
        }

        let mut new_body = format!("<!-- DIGEST:\n{summary}\n-->");
        if !kept_recent.is_empty() {
            new_body.push_str("\n\n");
            new_body.push_str(&kept_recent);
        }
        Ok(new_body)
    }
}

impl LlmClient {
    /// Stream `count` title candidates in a single API call. The model returns
    /// `# #1\n…\n# #2\n…` blocks; each block's body is one title.
    pub async fn stream_titles<F: FnMut(&str) + Send>(
        &self,
        prompts_dir: &Path,
        story: &Story,
        ideas: &[Idea],
        count: u32,
        cancel: Arc<AtomicBool>,
        mut on_delta: F,
    ) -> Result<String> {
        let count = count.max(1);
        let template = prompts::load_title_template(prompts_dir)?;
        let active = resolve_active_ideas(ideas, &story.meta.active_ideas);
        let ideas_block = format_ideas(&active);
        let body_block = if story.body.trim().is_empty() {
            "(empty draft)".to_string()
        } else {
            story.body.clone()
        };
        let count_str = count.to_string();
        let user_prompt = prompts::render(
            &template,
            &[
                ("ideas", &ideas_block),
                ("body", &body_block),
                ("count", &count_str),
            ],
        );

        let mut builder = CreateChatCompletionRequestArgs::default();
        builder
            .model(&self.cfg.model)
            .temperature(self.cfg.temperature)
            .top_p(self.cfg.top_p)
            .max_tokens(self.cfg.max_tokens)
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(user_prompt)
                .build()?
                .into()]);
        if self.cfg.disable_thinking {
            builder.reasoning_effort(ReasoningEffort::None);
        }
        let request = builder.build()?;

        let mut stream = self
            .client
            .chat()
            .create_stream(request)
            .await
            .context("title create_stream failed")?;

        let mut accumulated = String::new();
        let mut last_finish: Option<FinishReason> = None;
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                return Ok(accumulated);
            }
            let chunk = chunk.context("stream chunk error")?;
            for choice in chunk.choices {
                if let Some(reason) = choice.finish_reason {
                    last_finish = Some(reason);
                }
                if let Some(content) = choice.delta.content {
                    if !content.is_empty() {
                        accumulated.push_str(&content);
                        on_delta(&content);
                    }
                }
            }
        }

        let visible = visible_text(&accumulated).0;
        if visible.trim().is_empty() {
            let hint = match last_finish {
                Some(FinishReason::Length) => {
                    "max_tokens に達した。設定の max_tokens を増やしてください"
                }
                Some(FinishReason::ContentFilter) => {
                    "content_filter で削除された (NSFW 拒否)。別モデルを検討してください"
                }
                _ => "プロンプト形式やテンプレートを確認してください",
            };
            return Err(anyhow::anyhow!(
                "empty visible content (finish_reason={last_finish:?})\n→ {hint}"
            ));
        }
        Ok(accumulated)
    }
}

impl LlmClient {
    /// Stream a single fill-in proposal that covers every `<!-- FILL: ... -->`
    /// marker in `story.body`. The model receives the body with FILL markers
    /// replaced by `[FILL #N: hint]` and must answer with `# #N` blocks.
    /// Returns the raw accumulated text on success; the caller parses it with
    /// `parse_numbered_blocks` and applies it with `apply_fills`.
    pub async fn stream_fill<F: FnMut(&str) + Send>(
        &self,
        prompts_dir: &Path,
        story: &Story,
        ideas: &[Idea],
        slots: &[FillSlot],
        cancel: Arc<AtomicBool>,
        mut on_delta: F,
    ) -> Result<String> {
        if slots.is_empty() {
            return Err(anyhow::anyhow!("本文に `<!-- FILL: ... -->` がありません"));
        }
        let template = prompts::load_fill_template(prompts_dir)?;
        let active = resolve_active_ideas(ideas, &story.meta.active_ideas);
        let ideas_block = format_ideas(&active);
        let body_block = body_with_numbered_markers(&story.body, slots);
        let user_prompt = prompts::render(
            &template,
            &[("ideas", &ideas_block), ("body", &body_block)],
        );

        let mut builder = CreateChatCompletionRequestArgs::default();
        builder
            .model(&self.cfg.model)
            .temperature(self.cfg.temperature)
            .top_p(self.cfg.top_p)
            .max_tokens(self.cfg.max_tokens)
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(user_prompt)
                .build()?
                .into()]);
        if self.cfg.disable_thinking {
            builder.reasoning_effort(ReasoningEffort::None);
        }
        let request = builder.build()?;

        let mut stream = self
            .client
            .chat()
            .create_stream(request)
            .await
            .context("fill create_stream failed")?;

        let mut accumulated = String::new();
        let mut last_finish: Option<FinishReason> = None;
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                return Ok(accumulated);
            }
            let chunk = chunk.context("stream chunk error")?;
            for choice in chunk.choices {
                if let Some(reason) = choice.finish_reason {
                    last_finish = Some(reason);
                }
                if let Some(content) = choice.delta.content {
                    if !content.is_empty() {
                        accumulated.push_str(&content);
                        on_delta(&content);
                    }
                }
            }
        }

        let visible = visible_text(&accumulated).0;
        if visible.trim().is_empty() {
            let hint = match last_finish {
                Some(FinishReason::Length) => {
                    "max_tokens に達した。設定を見直してください"
                }
                Some(FinishReason::ContentFilter) => {
                    "content_filter で削除された (NSFW 拒否)。別モデルを検討してください"
                }
                _ => "プロンプト形式やテンプレートを確認してください",
            };
            return Err(anyhow::anyhow!(
                "empty visible content (finish_reason={last_finish:?})\n→ {hint}"
            ));
        }
        Ok(accumulated)
    }
}

/// Split `body` into `(to_summarize, kept_recent)`. The split point is at most
/// `keep_recent_chars` characters from the end, then snapped forward to the
/// next paragraph break (`\n\n`) so prose is not cut mid-sentence.
pub fn split_for_compaction(body: &str, keep_recent_chars: usize) -> (String, String) {
    let total_chars = body.chars().count();
    if total_chars <= keep_recent_chars {
        return (body.to_string(), String::new());
    }
    let split_idx_chars = total_chars - keep_recent_chars;
    let split_byte = body
        .char_indices()
        .nth(split_idx_chars)
        .map(|(i, _)| i)
        .unwrap_or(body.len());
    let after = &body[split_byte..];
    if let Some(rel) = after.find("\n\n") {
        let real_split = split_byte + rel + 2;
        (
            body[..real_split].trim_end().to_string(),
            body[real_split..].trim_start().to_string(),
        )
    } else {
        (
            body[..split_byte].to_string(),
            body[split_byte..].trim_start().to_string(),
        )
    }
}

/// A `<!-- FILL: hint -->` placeholder slot found in the body.
#[derive(Debug, Clone)]
pub struct FillSlot {
    /// Byte range of the entire `<!-- FILL: ... -->` marker in the original body.
    pub range: std::ops::Range<usize>,
    /// Hint text after the colon (trimmed). Empty when the user wrote `<!-- FILL: -->`.
    pub hint: String,
}

/// Find every `<!-- FILL: hint -->` marker in `body`, in document order.
pub fn extract_fills(body: &str) -> Vec<FillSlot> {
    let mut slots = Vec::new();
    let mut i = 0;
    while let Some(rel) = body[i..].find("<!--") {
        let start = i + rel;
        let Some(rel_end) = body[start..].find("-->") else { break; };
        let end = start + rel_end + 3;
        let inside = &body[start + 4..start + rel_end];
        let trimmed = inside.trim_start();
        if let Some(rest) = trimmed.strip_prefix("FILL") {
            if let Some(hint_part) = rest.trim_start().strip_prefix(':') {
                slots.push(FillSlot {
                    range: start..end,
                    hint: hint_part.trim().to_string(),
                });
            }
        }
        i = end;
    }
    slots
}

/// Render `body` with each FILL slot replaced by `[FILL #N: hint]` (or `[FILL #N]`
/// when the hint is empty). The numbering matches the order of `slots`.
pub fn body_with_numbered_markers(body: &str, slots: &[FillSlot]) -> String {
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for (i, slot) in slots.iter().enumerate() {
        out.push_str(&body[last..slot.range.start]);
        let n = i + 1;
        if slot.hint.is_empty() {
            out.push_str(&format!("[FILL #{n}]"));
        } else {
            out.push_str(&format!("[FILL #{n}: {}]", slot.hint));
        }
        last = slot.range.end;
    }
    out.push_str(&body[last..]);
    out
}

/// Parse `# #N`-delimited blocks (used for FILL, multi-proposals, and
/// auto-batch). Each block's body is the lines after `# #N` up to the next
/// header or end of input. Returns a map from N to body.
///
/// `#` (h1) is used rather than `##` (h2) because h1 is reserved for document
/// titles in Markdown, so it almost never appears mid-prose — making it a
/// safer delimiter that won't collide with the body's own headings.
pub fn parse_numbered_blocks(text: &str) -> std::collections::HashMap<usize, String> {
    let mut map = std::collections::HashMap::new();
    let mut current: Option<(usize, String)> = None;
    let flush = |cur: &mut Option<(usize, String)>, map: &mut std::collections::HashMap<usize, String>| {
        if let Some((n, buf)) = cur.take() {
            let trimmed = buf.trim().to_string();
            if !trimmed.is_empty() {
                map.insert(n, trimmed);
            }
        }
    };
    for line in text.lines() {
        let t = line.trim_start();
        // Match exactly one `#` (h1), not `##`/`###` etc.
        if let Some(rest) = t.strip_prefix('#') {
            if !rest.starts_with('#') {
                let rest = rest.trim_start();
                if let Some(num_part) = rest.strip_prefix('#') {
                    let digit_end = num_part.find(|c: char| !c.is_ascii_digit()).unwrap_or(num_part.len());
                    if digit_end > 0 {
                        if let Ok(n) = num_part[..digit_end].parse::<usize>() {
                            flush(&mut current, &mut map);
                            current = Some((n, String::new()));
                            continue;
                        }
                    }
                }
            }
        }
        if let Some((_, buf)) = current.as_mut() {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(line);
        }
    }
    flush(&mut current, &mut map);
    map
}

/// Substitute each FILL slot in `body` with the prose from `fills` (keyed by
/// the 1-based slot number). Slots without a corresponding entry are left
/// as-is (the original `<!-- FILL: ... -->` marker is preserved).
pub fn apply_fills(
    body: &str,
    slots: &[FillSlot],
    fills: &std::collections::HashMap<usize, String>,
) -> String {
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for (i, slot) in slots.iter().enumerate() {
        out.push_str(&body[last..slot.range.start]);
        let n = i + 1;
        match fills.get(&n) {
            Some(text) => out.push_str(text),
            None => out.push_str(&body[slot.range.clone()]),
        }
        last = slot.range.end;
    }
    out.push_str(&body[last..]);
    out
}

/// Strip well-formed `<!-- ... -->` HTML comments (author's notes) from text.
/// Defensive: prompts already tell the model not to emit them, but a misbehaving
/// model occasionally echoes them. Unclosed `<!--` is left as-is so the user
/// notices the mismatch.
pub fn strip_author_notes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let Some(start) = rest.find("<!--") else {
            out.push_str(rest);
            return out;
        };
        let Some(rel_end) = rest[start..].find("-->") else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..start]);
        rest = &rest[start + rel_end + 3..];
    }
}

/// Strip `<think>...</think>` blocks from raw model output for display.
/// Returns `(visible_text, currently_inside_unclosed_think_block)`.
pub fn visible_text(raw: &str) -> (String, bool) {
    let mut out = String::new();
    let mut s = raw;
    let mut in_think = false;
    loop {
        if in_think {
            match s.find("</think>") {
                Some(idx) => {
                    s = &s[idx + "</think>".len()..];
                    in_think = false;
                }
                None => return (out, true),
            }
        } else {
            match s.find("<think>") {
                Some(idx) => {
                    out.push_str(&s[..idx]);
                    s = &s[idx + "<think>".len()..];
                    in_think = true;
                }
                None => {
                    out.push_str(s);
                    return (out, false);
                }
            }
        }
    }
}

/// Expand the user's explicit picks with `requires`-driven auto-activations.
/// An idea auto-activates iff every id in its `requires` is already in the active set
/// (transitively, until fixed point).
pub fn resolve_active_ideas<'a>(all: &'a [Idea], explicit: &[Ulid]) -> Vec<&'a Idea> {
    let mut active: HashSet<Ulid> = explicit.iter().copied().collect();
    loop {
        let mut added = false;
        for idea in all {
            if active.contains(&idea.meta.id) || idea.meta.requires.is_empty() {
                continue;
            }
            if idea.meta.requires.iter().all(|r| active.contains(r)) {
                active.insert(idea.meta.id);
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    all.iter().filter(|i| active.contains(&i.meta.id)).collect()
}

fn format_ideas(ideas: &[&Idea]) -> String {
    if ideas.is_empty() {
        return "(none)".to_string();
    }
    let mut out = String::new();
    for idea in ideas {
        out.push_str(&format!("## {}\n{}\n\n", idea.meta.title, idea.body.trim()));
    }
    out.trim_end().to_string()
}

fn placeholder_empty_body() -> &'static str {
    "(No draft yet. Begin from the opening.)"
}
