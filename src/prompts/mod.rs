use crate::data::story::Language;
use anyhow::{Context, Result};
use std::path::Path;

const NSFW_JA: &str = include_str!("defaults/nsfw_ja.md");
const NSFW_EN: &str = include_str!("defaults/nsfw_en.md");
const SFW_JA: &str = include_str!("defaults/sfw_ja.md");
const SFW_EN: &str = include_str!("defaults/sfw_en.md");

#[derive(Debug, Clone, Copy)]
pub struct PromptKey {
    pub language: Language,
    pub nsfw: bool,
}

impl PromptKey {
    pub fn filename(self) -> &'static str {
        match (self.nsfw, self.language) {
            (true, Language::Ja) => "nsfw_ja.md",
            (true, Language::En) => "nsfw_en.md",
            (false, Language::Ja) => "sfw_ja.md",
            (false, Language::En) => "sfw_en.md",
        }
    }

    fn default_template(self) -> &'static str {
        match (self.nsfw, self.language) {
            (true, Language::Ja) => NSFW_JA,
            (true, Language::En) => NSFW_EN,
            (false, Language::Ja) => SFW_JA,
            (false, Language::En) => SFW_EN,
        }
    }
}

/// Load the prompt template for `key`. If `<prompts_dir>/<filename>` exists,
/// it overrides the bundled default.
pub fn load_template(prompts_dir: &Path, key: PromptKey) -> Result<String> {
    let override_path = prompts_dir.join(key.filename());
    if override_path.exists() {
        return std::fs::read_to_string(&override_path)
            .with_context(|| format!("read override prompt {override_path:?}"));
    }
    Ok(key.default_template().to_string())
}

/// Render a template by substituting `{{key}}` placeholders.
pub fn render(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}
