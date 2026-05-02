use anyhow::{Context, Result};
use std::path::Path;

const NSFW: &str = include_str!("defaults/nsfw.md");
const SFW: &str = include_str!("defaults/sfw.md");
const COMPACT: &str = include_str!("defaults/compact.md");
const FILL: &str = include_str!("defaults/fill.md");

#[derive(Debug, Clone, Copy)]
pub struct PromptKey {
    pub nsfw: bool,
}

impl PromptKey {
    pub fn filename(self) -> &'static str {
        if self.nsfw { "nsfw.md" } else { "sfw.md" }
    }

    fn default_template(self) -> &'static str {
        if self.nsfw { NSFW } else { SFW }
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

pub fn load_compact_template(prompts_dir: &Path) -> Result<String> {
    let override_path = prompts_dir.join("compact.md");
    if override_path.exists() {
        return std::fs::read_to_string(&override_path)
            .with_context(|| format!("read override prompt {override_path:?}"));
    }
    Ok(COMPACT.to_string())
}

pub fn load_fill_template(prompts_dir: &Path) -> Result<String> {
    let override_path = prompts_dir.join("fill.md");
    if override_path.exists() {
        return std::fs::read_to_string(&override_path)
            .with_context(|| format!("read override prompt {override_path:?}"));
    }
    Ok(FILL.to_string())
}
