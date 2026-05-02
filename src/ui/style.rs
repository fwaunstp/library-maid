pub const CSS: &str = r#"
* { box-sizing: border-box; }
body, html { margin: 0; padding: 0; height: 100%; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Hiragino Sans", "Yu Gothic UI", sans-serif;
  background: #1d1f23;
  color: #e6e6e6;
  font-size: 14px;
}
button {
  background: #2e323a;
  color: #e6e6e6;
  border: 1px solid #3b414c;
  padding: 4px 10px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
button:hover { background: #3a3f48; }
button.primary { background: #4a6cf7; border-color: #4a6cf7; }
button.primary:hover { background: #5a7cff; }
button.danger { background: #6b2222; border-color: #8a2a2a; }
button.danger:hover { background: #8a2a2a; }
button:disabled { opacity: 0.5; cursor: not-allowed; }
input, textarea, select {
  background: #15171a;
  color: #e6e6e6;
  border: 1px solid #3b414c;
  border-radius: 4px;
  padding: 6px 8px;
  font-family: inherit;
  font-size: 13px;
}
textarea { resize: vertical; }

.app-root { display: flex; flex-direction: column; height: 100vh; }

/* Setup screen */
.setup {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  height: 100vh; gap: 16px; padding: 24px; text-align: center;
}
.setup h1 { margin: 0; }
.setup p { color: #9aa0aa; max-width: 480px; }

/* Library */
.library { display: grid; grid-template-columns: 240px 320px 1fr; height: 100vh; }
.tabs { display: flex; flex-direction: column; background: #16181c; border-right: 1px solid #2a2d33; }
.tabs button {
  background: transparent; border: none; border-radius: 0; text-align: left;
  padding: 14px 18px; color: #c4c8d0; border-bottom: 1px solid #1f2227;
}
.tabs button.active { background: #23262d; color: #fff; border-left: 3px solid #4a6cf7; }
.tabs .footer { margin-top: auto; padding: 12px 18px; font-size: 11px; color: #6b6f78; word-break: break-all; }

.list { background: #1a1c20; border-right: 1px solid #2a2d33; overflow-y: auto; }
.list-toolbar { display: flex; padding: 8px; gap: 6px; border-bottom: 1px solid #2a2d33; position: sticky; top: 0; background: #1a1c20; }
.list-item {
  padding: 10px 12px; border-bottom: 1px solid #23262d; cursor: pointer;
  display: flex; flex-direction: column; gap: 2px;
}
.list-item:hover { background: #23262d; }
.list-item.selected { background: #2e3340; }
.list-item .title { color: #e6e6e6; }
.list-item .sub { color: #8a8f99; font-size: 11px; }

.editor { display: flex; flex-direction: column; padding: 16px; gap: 12px; overflow: hidden; }
.editor .row { display: flex; gap: 8px; align-items: center; }
.editor .row label { min-width: 80px; color: #9aa0aa; }
.editor .row input[type=text] { flex: 1; }
.editor textarea { width: 100%; }
.editor h2 { margin: 0 0 8px; }
.empty { color: #6b6f78; padding: 32px; text-align: center; }

.story-layout { display: grid; grid-template-rows: auto auto 1fr auto; height: 100%; gap: 12px; overflow: hidden; min-height: 0; }
.idea-toggles { max-height: 96px; overflow-y: auto; align-content: flex-start; }
.story-meta { display: grid; grid-template-columns: 1fr auto auto auto auto; gap: 8px; align-items: center; }
.story-body { flex: 1; min-height: 0; display: flex; }
.story-body textarea { flex: 1; height: 100%; min-height: 0; font-family: ui-monospace, "SF Mono", Menlo, monospace; line-height: 1.6; }
.gen-panel { display: flex; flex-direction: column; gap: 8px; max-height: 50vh; overflow-y: auto; }
.gen-controls { display: flex; gap: 8px; align-items: center; padding: 8px; background: #15171a; border: 1px solid #2a2d33; border-radius: 4px; }
.gen-controls input[type=number] { width: 60px; }
.proposal {
  background: #1f2228; border: 1px solid #2a2d33; border-radius: 4px;
  padding: 10px; display: flex; flex-direction: column; gap: 8px;
}
.proposal .text { white-space: pre-wrap; line-height: 1.6; color: #d4d8e0; }
.proposal .actions { display: flex; gap: 6px; justify-content: flex-end; }
.proposal.pending { opacity: 0.6; }

.idea-toggles { display: flex; flex-wrap: wrap; gap: 6px; padding: 4px 0; }
.idea-toggles .chip {
  background: #2a2d33; border: 1px solid #3b414c; border-radius: 999px;
  padding: 3px 10px; cursor: pointer; font-size: 12px;
}
.idea-toggles .chip.on { background: #4a6cf7; border-color: #4a6cf7; color: #fff; }
.idea-toggles .chip.auto { background: #2a4030; border-color: #3a6048; color: #b9e8c8; cursor: default; }

.tags-input { display: flex; gap: 6px; flex-wrap: wrap; }
.tags-input .tag {
  background: #2a2d33; border-radius: 999px; padding: 2px 8px; font-size: 11px;
  display: inline-flex; gap: 4px; align-items: center;
}
.tags-input .tag button { padding: 0 4px; font-size: 10px; }

.status-bar {
  background: #15171a; border-top: 1px solid #2a2d33; padding: 4px 12px;
  font-size: 11px; color: #8a8f99;
}
"#;
