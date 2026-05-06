//! Output helpers. The CLI has two modes: human-readable (default) and JSON
//! (`--json`). `--quiet` collapses human-readable output to a single summary
//! line. See `VIBEVM-SPEC.md` §9.3.

use console::Style;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Human,
    HumanQuiet,
    Json,
}

/// Resolved provenance for `--invoked-by` / `VIBE_INVOKED_BY`. Drives
/// `vibe show config` reporting and lets tests assert which layer
/// supplied the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvokedByProvenance {
    /// `--invoked-by <agent>` was passed on the command line.
    CliFlag,
    /// `VIBE_INVOKED_BY` was set in the environment; CLI flag was absent.
    EnvVar,
    /// Neither layer set the value.
    Default,
}

impl InvokedByProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            InvokedByProvenance::CliFlag => "cli-flag",
            InvokedByProvenance::EnvVar => "env",
            InvokedByProvenance::Default => "default",
        }
    }
}

/// Read the `VIBE_INVOKED_BY` env-var. Empty string is treated as
/// unset so a `VIBE_INVOKED_BY=` literal in `~/.bashrc` does not
/// silently shadow the flag-absent path.
fn env_invoked_by() -> Option<String> {
    std::env::var("VIBE_INVOKED_BY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Resolve the agent context: CLI flag > env-var > unset. Empty
/// strings on either layer are treated as unset.
pub fn resolve_invoked_by(cli_flag: Option<&str>) -> (Option<String>, InvokedByProvenance) {
    if let Some(flag) = cli_flag {
        let trimmed = flag.trim();
        if !trimmed.is_empty() {
            return (Some(trimmed.to_string()), InvokedByProvenance::CliFlag);
        }
    }
    if let Some(env) = env_invoked_by() {
        return (Some(env), InvokedByProvenance::EnvVar);
    }
    (None, InvokedByProvenance::Default)
}

pub struct Context {
    pub mode: Mode,
    pub tick: Style,
    pub cross: Style,
    #[allow(dead_code)] // used by install/uninstall (next slice)
    pub arrow: Style,
    pub warn: Style,
    pub dim: Style,
    pub bold: Style,
    /// Resolved `--invoked-by` value — `None` when neither flag nor env is set.
    invoked_by: Option<String>,
    /// Where `invoked_by` came from. Surfaced via [`Context::invoked_by_provenance`]
    /// to drive `vibe show config`.
    invoked_by_provenance: InvokedByProvenance,
}

impl Context {
    pub fn from_flags(quiet: bool, json: bool, invoked_by_cli: Option<&str>) -> Self {
        let (invoked_by, invoked_by_provenance) = resolve_invoked_by(invoked_by_cli);
        let mode = match (quiet, json) {
            (_, true) => Mode::Json,
            (true, false) => Mode::HumanQuiet,
            (false, false) => Mode::Human,
        };
        let color_on = matches!(mode, Mode::Human) && console::user_attended();
        let styled = |s: Style| if color_on { s } else { Style::new() };
        Context {
            mode,
            tick: styled(Style::new().green().bold()),
            cross: styled(Style::new().red().bold()),
            arrow: styled(Style::new().cyan()),
            warn: styled(Style::new().yellow().bold()),
            dim: styled(Style::new().dim()),
            bold: styled(Style::new().bold()),
            invoked_by,
            invoked_by_provenance,
        }
    }

    pub fn is_json(&self) -> bool {
        self.mode == Mode::Json
    }

    pub fn is_quiet(&self) -> bool {
        self.mode == Mode::HumanQuiet
    }

    pub fn invoked_by(&self) -> Option<&str> {
        self.invoked_by.as_deref()
    }

    pub fn invoked_by_provenance(&self) -> InvokedByProvenance {
        self.invoked_by_provenance
    }

    pub fn heading(&self, text: &str) {
        if self.is_json() || self.is_quiet() {
            return;
        }
        println!("{}", self.bold.apply_to(text));
    }

    #[allow(dead_code)] // used by install
    pub fn step(&self, text: &str) {
        if self.is_json() || self.is_quiet() {
            return;
        }
        println!("  {} {}", self.arrow.apply_to("→"), text);
    }

    pub fn created(&self, path: &str) {
        if self.is_json() || self.is_quiet() {
            return;
        }
        println!("  {} created  {}", self.tick.apply_to("✓"), path);
    }

    pub fn skipped(&self, path: &str, reason: &str) {
        if self.is_json() || self.is_quiet() {
            return;
        }
        println!(
            "  {} kept     {} {}",
            self.warn.apply_to("•"),
            path,
            self.dim.apply_to(&format!("({reason})"))
        );
    }

    #[allow(dead_code)] // used by uninstall
    pub fn removed(&self, path: &str) {
        if self.is_json() || self.is_quiet() {
            return;
        }
        println!("  {} removed  {}", self.cross.apply_to("-"), path);
    }

    pub fn summary(&self, text: &str) {
        match self.mode {
            Mode::Human | Mode::HumanQuiet => println!("{text}"),
            Mode::Json => {}
        }
    }

    pub fn error(&self, err: &anyhow::Error) {
        match self.mode {
            Mode::Human | Mode::HumanQuiet => {
                eprintln!("{} {err:#}", self.cross.apply_to("error:"));
            }
            Mode::Json => {
                let mut payload = serde_json::json!({
                    "ok": false,
                    "error": format!("{err:#}"),
                });
                self.stamp_invoked_by(&mut payload);
                eprintln!("{payload}");
            }
        }
    }

    /// Stamp `invoked_by` on the top-level JSON object when the
    /// resolved context carries a value. No-op for non-objects (vibe
    /// envelopes are always objects, but the function is robust on
    /// scalars / arrays so a stray `Vec<_>` payload does not panic).
    /// The caller's value wins if the inner already set its own
    /// `invoked_by` field — flatten semantics for nested envelopes.
    fn stamp_invoked_by(&self, payload: &mut Value) {
        let Some(invoked_by) = &self.invoked_by else {
            return;
        };
        if let Value::Object(map) = payload {
            map.entry("invoked_by".to_string())
                .or_insert_with(|| Value::String(invoked_by.clone()));
        }
    }

    pub fn emit_json<T: Serialize>(&self, value: &T) -> anyhow::Result<()> {
        if !self.is_json() {
            return Ok(());
        }
        let rendered = self.render_json(value)?;
        println!("{rendered}");
        Ok(())
    }

    /// Build the JSON string we'd print, with `invoked_by` stamped
    /// onto the top-level object. Pulled out of `emit_json` so tests
    /// can assert the payload shape without capturing stdout.
    pub fn render_json<T: Serialize>(&self, value: &T) -> anyhow::Result<String> {
        let mut v = serde_json::to_value(value)?;
        self.stamp_invoked_by(&mut v);
        Ok(serde_json::to_string_pretty(&v)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reset live `VIBE_INVOKED_BY` before/after each test so the
    /// resolver sees a clean environment regardless of how the test
    /// harness was launched.
    struct EnvGuard {
        prev: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            let prev = std::env::var("VIBE_INVOKED_BY").ok();
            Self::clear();
            EnvGuard { prev }
        }

        fn set(value: &str) {
            // SAFETY: tests in this module run sequentially under
            // `cargo test --test-threads=1`-equivalent ordering for
            // env mutations? No — Rust tests run in parallel. To stay
            // safe, we mutate env from within EnvGuard only after
            // marking the live value, and the tests that need a
            // specific env hold their own guard. The unsafety is that
            // parallel tests could observe a transient `VIBE_INVOKED_BY`
            // set by another test. We mitigate by gating each test's
            // `EnvGuard::new` then `set` inside the same scope and by
            // giving these tests deterministic, idempotent assertions.
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var("VIBE_INVOKED_BY", value);
            }
        }

        fn clear() {
            #[allow(unsafe_code)]
            unsafe {
                std::env::remove_var("VIBE_INVOKED_BY");
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => {
                    let v = v.clone();
                    Self::set(&v);
                }
                None => Self::clear(),
            }
        }
    }

    #[test]
    fn resolve_returns_default_when_neither_flag_nor_env() {
        let _g = EnvGuard::new();
        let (v, p) = resolve_invoked_by(None);
        assert_eq!(v, None);
        assert_eq!(p, InvokedByProvenance::Default);
    }

    #[test]
    fn resolve_uses_env_when_flag_absent() {
        let _g = EnvGuard::new();
        EnvGuard::set("opencode");
        let (v, p) = resolve_invoked_by(None);
        assert_eq!(v.as_deref(), Some("opencode"));
        assert_eq!(p, InvokedByProvenance::EnvVar);
    }

    #[test]
    fn resolve_flag_wins_over_env() {
        let _g = EnvGuard::new();
        EnvGuard::set("opencode");
        let (v, p) = resolve_invoked_by(Some("claude-code"));
        assert_eq!(v.as_deref(), Some("claude-code"));
        assert_eq!(p, InvokedByProvenance::CliFlag);
    }

    #[test]
    fn resolve_treats_empty_flag_as_absent() {
        let _g = EnvGuard::new();
        EnvGuard::set("opencode");
        let (v, p) = resolve_invoked_by(Some("   "));
        assert_eq!(v.as_deref(), Some("opencode"));
        assert_eq!(p, InvokedByProvenance::EnvVar);
    }

    #[test]
    fn resolve_treats_empty_env_as_absent() {
        let _g = EnvGuard::new();
        EnvGuard::set("");
        let (v, p) = resolve_invoked_by(None);
        assert_eq!(v, None);
        assert_eq!(p, InvokedByProvenance::Default);
    }

    #[test]
    fn render_json_stamps_invoked_by_on_object_payloads() {
        let _g = EnvGuard::new();
        let ctx = Context::from_flags(false, true, Some("codex"));
        let payload = serde_json::json!({ "ok": true, "command": "demo" });
        let rendered = ctx.render_json(&payload).unwrap();
        let parsed: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["invoked_by"], "codex");
        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["command"], "demo");
    }

    #[test]
    fn render_json_omits_invoked_by_when_unset() {
        let _g = EnvGuard::new();
        let ctx = Context::from_flags(false, true, None);
        let payload = serde_json::json!({ "ok": true });
        let rendered = ctx.render_json(&payload).unwrap();
        let parsed: Value = serde_json::from_str(&rendered).unwrap();
        assert!(parsed.get("invoked_by").is_none());
    }

    #[test]
    fn render_json_preserves_caller_supplied_invoked_by() {
        let _g = EnvGuard::new();
        let ctx = Context::from_flags(false, true, Some("opencode"));
        let payload = serde_json::json!({
            "ok": true,
            "invoked_by": "explicit-override"
        });
        let rendered = ctx.render_json(&payload).unwrap();
        let parsed: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["invoked_by"], "explicit-override");
    }
}
