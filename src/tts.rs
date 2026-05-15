use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtsConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub voice: String,
    pub speed: f32,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            api_base: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model: "gpt-4o-mini-tts".into(),
            voice: "alloy".into(),
            speed: 1.0,
        }
    }
}

impl TtsConfig {
    /// POST /audio/speech and return the raw mp3 bytes.
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        if self.api_key.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "TTS API キーが未設定です。設定 → TTS 接続 で入力してください"
            ));
        }
        let url = format!("{}/audio/speech", self.api_base.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "voice": self.voice,
            "input": text,
            "response_format": "mp3",
            "speed": self.speed,
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        let resp = client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("{status}: {detail}"));
        }
        let bytes = resp.bytes().await.context("read audio body")?;
        Ok(bytes.to_vec())
    }
}

/// Decode the MP3 bytes and play through the default output device.
/// Polls `cancel` ~20Hz and returns early when set or playback finishes.
/// Must be called from a blocking context (uses `std::thread::sleep`).
pub fn play_blocking(bytes: Vec<u8>, cancel: Arc<AtomicBool>) -> Result<()> {
    // rodio 0.22 API: open the default mixer, then attach a Player to it.
    let device = rodio::DeviceSinkBuilder::open_default_sink()
        .context("open default audio output")?;
    let player = rodio::Player::connect_new(device.mixer());
    let source = rodio::Decoder::new(Cursor::new(bytes)).context("decode mp3")?;
    player.append(source);
    while !player.empty() {
        if cancel.load(Ordering::SeqCst) {
            player.stop();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

/// Pick the substring to read aloud, in priority order:
/// 1. non-empty selection
/// 2. paragraph containing `cursor` (blocks separated by blank lines)
/// 3. line containing `cursor`
///
/// `selection` and `cursor` are *character* indices (egui's `CCursor.index`).
/// Returns `None` only when the body is fully empty or whitespace.
pub fn pick_speech_text(
    body: &str,
    selection: Option<(usize, usize)>,
    cursor: Option<usize>,
) -> Option<String> {
    if let Some((a, b)) = selection {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        if hi > lo {
            let s: String = body.chars().skip(lo).take(hi - lo).collect();
            if !s.trim().is_empty() {
                return Some(s);
            }
        }
    }
    let cur_char = cursor.unwrap_or(0);
    let cur_byte = char_to_byte(body, cur_char);

    if let Some(p) = paragraph_at(body, cur_byte) {
        if !p.trim().is_empty() {
            return Some(p);
        }
    }
    if let Some(l) = line_at(body, cur_byte) {
        if !l.trim().is_empty() {
            return Some(l);
        }
    }
    None
}

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Find the paragraph (text between blank lines) that contains byte position `pos`.
fn paragraph_at(s: &str, pos: usize) -> Option<String> {
    let pos = pos.min(s.len());
    let start = s[..pos].rfind("\n\n").map(|i| i + 2).unwrap_or(0);
    let end = s[pos..]
        .find("\n\n")
        .map(|i| pos + i)
        .unwrap_or(s.len());
    let para = s[start..end].trim();
    if para.is_empty() { None } else { Some(para.to_string()) }
}

fn line_at(s: &str, pos: usize) -> Option<String> {
    let pos = pos.min(s.len());
    let start = s[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = s[pos..].find('\n').map(|i| pos + i).unwrap_or(s.len());
    let line = s[start..end].trim();
    if line.is_empty() { None } else { Some(line.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph_picks_block_at_cursor() {
        let body = "first para line1\nfirst para line2\n\nsecond para\n\nthird";
        // cursor in middle of second para
        let pos = body.find("second").unwrap() + 2;
        assert_eq!(paragraph_at(body, pos).as_deref(), Some("second para"));
    }

    #[test]
    fn selection_wins_over_paragraph() {
        let body = "abc def ghi";
        let got = pick_speech_text(body, Some((4, 7)), Some(0));
        assert_eq!(got.as_deref(), Some("def"));
    }

    #[test]
    fn empty_selection_falls_back_to_paragraph() {
        let body = "one\n\ntwo three";
        let pos_in_two = body.find("two").unwrap();
        let cur_char = body[..pos_in_two].chars().count();
        let got = pick_speech_text(body, Some((cur_char, cur_char)), Some(cur_char));
        assert_eq!(got.as_deref(), Some("two three"));
    }
}
