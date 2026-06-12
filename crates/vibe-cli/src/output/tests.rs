//! Unit tests for the output context. Split out of `output.rs` so the
//! production file stays inside the file-length budget. The env-var
//! guards and their serialisation locks stay in `output.rs` next to
//! the production code (the unsafe-gate baseline keys their `unsafe`
//! blocks by file) and arrive here via `use super::*`.

specmark::scope!("spec://vibevm/VIBEVM-SPEC#output-format");

use super::*;

#[test]
fn resolve_returns_default_when_neither_flag_nor_env() {
    let _lock = INVOKED_BY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = EnvGuard::new();
    let (v, p) = resolve_invoked_by(None);
    assert_eq!(v, None);
    assert_eq!(p, InvokedByProvenance::Default);
}

#[test]
fn resolve_uses_env_when_flag_absent() {
    let _lock = INVOKED_BY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = EnvGuard::new();
    EnvGuard::set("opencode");
    let (v, p) = resolve_invoked_by(None);
    assert_eq!(v.as_deref(), Some("opencode"));
    assert_eq!(p, InvokedByProvenance::EnvVar);
}

#[test]
fn resolve_flag_wins_over_env() {
    let _lock = INVOKED_BY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = EnvGuard::new();
    EnvGuard::set("opencode");
    let (v, p) = resolve_invoked_by(Some("claude-code"));
    assert_eq!(v.as_deref(), Some("claude-code"));
    assert_eq!(p, InvokedByProvenance::CliFlag);
}

#[test]
fn resolve_treats_empty_flag_as_absent() {
    let _lock = INVOKED_BY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = EnvGuard::new();
    EnvGuard::set("opencode");
    let (v, p) = resolve_invoked_by(Some("   "));
    assert_eq!(v.as_deref(), Some("opencode"));
    assert_eq!(p, InvokedByProvenance::EnvVar);
}

#[test]
fn resolve_treats_empty_env_as_absent() {
    let _lock = INVOKED_BY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = EnvGuard::new();
    EnvGuard::set("");
    let (v, p) = resolve_invoked_by(None);
    assert_eq!(v, None);
    assert_eq!(p, InvokedByProvenance::Default);
}

#[test]
fn render_json_stamps_invoked_by_on_object_payloads() {
    let _lock = INVOKED_BY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = EnvGuard::new();
    let ctx = Context::from_flags(false, true, Some("codex"), false);
    let payload = serde_json::json!({ "ok": true, "command": "demo" });
    let rendered = ctx.render_json(&payload).unwrap();
    let parsed: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed["invoked_by"], "codex");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["command"], "demo");
}

#[test]
fn render_json_omits_invoked_by_when_unset() {
    let _lock = INVOKED_BY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = EnvGuard::new();
    let ctx = Context::from_flags(false, true, None, false);
    let payload = serde_json::json!({ "ok": true });
    let rendered = ctx.render_json(&payload).unwrap();
    let parsed: Value = serde_json::from_str(&rendered).unwrap();
    assert!(parsed.get("invoked_by").is_none());
}

#[test]
fn unattended_default_false_with_no_flag_no_env() {
    let _lock = UNATTENDED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = UnattendedGuard::new();
    assert!(!resolve_unattended(false));
}

#[test]
fn unattended_cli_flag_true_wins() {
    let _lock = UNATTENDED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = UnattendedGuard::new();
    assert!(resolve_unattended(true));
}

#[test]
fn unattended_env_truthy_values() {
    let _lock = UNATTENDED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for raw in ["1", "true", "TRUE", " yes ", "On", "yes"] {
        let _g = UnattendedGuard::new();
        UnattendedGuard::set(raw);
        assert!(
            resolve_unattended(false),
            "VIBE_UNATTENDED={raw:?} must resolve to true"
        );
    }
}

#[test]
fn unattended_env_falsy_values_or_empty_or_unset() {
    let _lock = UNATTENDED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for raw in ["", "0", "false", "no", "off", "garbage", "  "] {
        let _g = UnattendedGuard::new();
        UnattendedGuard::set(raw);
        assert!(
            !resolve_unattended(false),
            "VIBE_UNATTENDED={raw:?} must resolve to false"
        );
    }
}

#[test]
fn unattended_cli_flag_overrides_falsy_env() {
    let _lock = UNATTENDED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = UnattendedGuard::new();
    UnattendedGuard::set("0");
    // Flag is true, env is falsy → resolved is true (flag wins by OR).
    assert!(resolve_unattended(true));
}

#[test]
fn render_json_stamps_unattended_when_true() {
    let _lock = UNATTENDED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g_inv = EnvGuard::new();
    let _g_un = UnattendedGuard::new();
    let ctx = Context::from_flags(false, true, None, true);
    let payload = serde_json::json!({ "ok": true, "command": "demo" });
    let rendered = ctx.render_json(&payload).unwrap();
    let parsed: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed["unattended"], true);
    assert_eq!(parsed["ok"], true);
}

#[test]
fn render_json_omits_unattended_when_false() {
    let _lock = UNATTENDED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g_inv = EnvGuard::new();
    let _g_un = UnattendedGuard::new();
    let ctx = Context::from_flags(false, true, None, false);
    let payload = serde_json::json!({ "ok": true });
    let rendered = ctx.render_json(&payload).unwrap();
    let parsed: Value = serde_json::from_str(&rendered).unwrap();
    assert!(parsed.get("unattended").is_none());
}

#[test]
fn render_json_preserves_caller_supplied_invoked_by() {
    let _lock = INVOKED_BY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _g = EnvGuard::new();
    let ctx = Context::from_flags(false, true, Some("opencode"), false);
    let payload = serde_json::json!({
        "ok": true,
        "invoked_by": "explicit-override"
    });
    let rendered = ctx.render_json(&payload).unwrap();
    let parsed: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed["invoked_by"], "explicit-override");
}
