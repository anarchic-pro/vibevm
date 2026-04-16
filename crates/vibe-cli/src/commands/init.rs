//! `vibe init` — scaffold a new vibevm project.
//!
//! Spec: `VIBEVM-SPEC.md` §9.1, §11.1.
//! Acceptance: the produced tree matches §4.2; running twice does not destroy
//! user-modified files (idempotent).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use vibe_core::manifest::{
    ActiveSection, Lockfile, ProjectManifest, ProjectSection,
};

use crate::cli::InitArgs;
use crate::output;

const REDIRECT_LINE: &str = "Read every file in spec/boot/ in filename order, then await the user's instructions.\n";

pub fn run(ctx: &output::Context, args: InitArgs) -> Result<()> {
    fs::create_dir_all(&args.path)
        .with_context(|| format!("creating project directory `{}`", args.path.display()))?;

    let path = canonical_no_unc(&args.path)?;
    let display_root = normalize_display(&args.path, &path);

    if !path.is_dir() {
        bail!("target `{}` is not a directory", display_root);
    }

    let project_name = resolve_name(&args, &path)?;

    ctx.heading(&format!(
        "Initializing project `{project_name}` in `{display_root}`"
    ));

    let mut outcomes = Vec::<Outcome>::new();

    // 1. Redirect files (CLAUDE.md, AGENTS.md, GEMINI.md).
    for filename in ["CLAUDE.md", "AGENTS.md", "GEMINI.md"] {
        outcomes.push(ensure_file(
            ctx,
            &path,
            &path.join(filename),
            REDIRECT_LINE,
            "agent redirect",
        )?);
    }

    // 2. spec/ directory tree.
    for sub in ["boot", "flows", "feats", "stacks", "common", "modules"] {
        ensure_dir(&path.join("spec").join(sub))?;
    }

    // 3. User-owned boot snippets.
    outcomes.push(ensure_file(
        ctx,
        &path,
        &path.join("spec/boot/00-core.md"),
        &boot_00_core_template(&project_name),
        "boot: project foundation",
    )?);
    outcomes.push(ensure_file(
        ctx,
        &path,
        &path.join("spec/boot/90-user.md"),
        BOOT_90_USER_TEMPLATE,
        "boot: user overrides",
    )?);

    // 4. WAL.
    outcomes.push(ensure_file(
        ctx,
        &path,
        &path.join("spec/WAL.md"),
        &wal_template(&project_name),
        "WAL checkpoint",
    )?);

    // 5. Project manifest and empty lockfile.
    outcomes.push(ensure_project_manifest(
        ctx,
        &path,
        &project_name,
        args.stack.as_deref(),
    )?);
    outcomes.push(ensure_empty_lockfile(ctx, &path)?);

    // 6. `.vibe/` cache (gitignored per §4.2).
    ensure_dir(&path.join(".vibe/cache"))?;
    outcomes.push(ensure_file(
        ctx,
        &path,
        &path.join(".vibe/.gitignore"),
        "*\n",
        "gitignore: cache",
    )?);

    // 7. .gitignore at project root (only if absent — don't overwrite).
    outcomes.push(ensure_file(
        ctx,
        &path,
        &path.join(".gitignore"),
        ROOT_GITIGNORE_TEMPLATE,
        "gitignore: root",
    )?);

    report(ctx, &project_name, &display_root, &outcomes)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct Outcome {
    path: String,
    action: Action,
    reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Created,
    Kept,
}

fn ensure_file(
    ctx: &output::Context,
    root: &Path,
    path: &Path,
    content: &str,
    reason: &'static str,
) -> Result<Outcome> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let rel = relative_to_root(root, path);
    if path.exists() {
        ctx.skipped(&rel, "already exists");
        return Ok(Outcome {
            path: rel,
            action: Action::Kept,
            reason,
        });
    }
    fs::write(path, content).with_context(|| format!("writing `{}`", path.display()))?;
    ctx.created(&rel);
    Ok(Outcome {
        path: rel,
        action: Action::Created,
        reason,
    })
}

fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("creating `{}`", path.display()))
}

fn ensure_project_manifest(
    ctx: &output::Context,
    root: &Path,
    name: &str,
    stack: Option<&str>,
) -> Result<Outcome> {
    let path = root.join(ProjectManifest::FILENAME);
    let rel = relative_to_root(root, &path);
    if path.exists() {
        ctx.skipped(&rel, "already exists");
        return Ok(Outcome {
            path: rel,
            action: Action::Kept,
            reason: "project manifest",
        });
    }

    let manifest = ProjectManifest {
        project: ProjectSection {
            name: name.to_string(),
            version: "0.0.1".to_string(),
            authors: vec![],
        },
        active: stack.map(|s| ActiveSection {
            stack: Some(s.to_string()),
        }),
        llm: None,
        registry: None,
    };

    manifest.write(&path)?;
    ctx.created(&rel);
    Ok(Outcome {
        path: rel,
        action: Action::Created,
        reason: "project manifest",
    })
}

fn ensure_empty_lockfile(ctx: &output::Context, root: &Path) -> Result<Outcome> {
    let path = root.join(Lockfile::FILENAME);
    let rel = relative_to_root(root, &path);
    if path.exists() {
        ctx.skipped(&rel, "already exists");
        return Ok(Outcome {
            path: rel,
            action: Action::Kept,
            reason: "lockfile",
        });
    }
    let lockfile = Lockfile::empty(
        format!("vibe {}", env!("CARGO_PKG_VERSION")),
        current_timestamp_utc(),
    );
    lockfile.write(&path)?;
    ctx.created(&rel);
    Ok(Outcome {
        path: rel,
        action: Action::Created,
        reason: "lockfile",
    })
}

fn resolve_name(args: &InitArgs, path: &Path) -> Result<String> {
    if let Some(n) = &args.name {
        return Ok(n.clone());
    }
    let basename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    Ok(basename.to_string())
}

fn relative_to_root(root: &Path, full: &Path) -> String {
    let stripped = full.strip_prefix(root).unwrap_or(full);
    display_pathbuf(stripped)
}

fn display_pathbuf(p: &Path) -> String {
    // Display with forward slashes — consistent across macOS/Linux/Windows.
    let s = p.display().to_string();
    s.replace('\\', "/")
}

/// Canonicalize and strip Windows UNC (`\\?\`) prefix where present.
fn canonical_no_unc(path: &Path) -> Result<std::path::PathBuf> {
    let canon = path
        .canonicalize()
        .with_context(|| format!("canonicalizing `{}`", path.display()))?;
    Ok(strip_unc(canon))
}

#[cfg(windows)]
fn strip_unc(p: std::path::PathBuf) -> std::path::PathBuf {
    let s = p.as_os_str().to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        std::path::PathBuf::from(rest)
    } else {
        p
    }
}

#[cfg(not(windows))]
fn strip_unc(p: std::path::PathBuf) -> std::path::PathBuf {
    p
}

/// Re-export for sibling command modules.
pub(crate) fn strip_unc_public(p: std::path::PathBuf) -> std::path::PathBuf {
    strip_unc(p)
}

/// Prefer the user-supplied display (e.g. `.`) if it still points at the
/// canonical path; otherwise fall back to the canonical (UNC-stripped) form.
fn normalize_display(requested: &Path, canonical: &Path) -> String {
    let requested_matches = requested
        .canonicalize()
        .map(|c| strip_unc(c) == *canonical)
        .unwrap_or(false);
    if requested_matches {
        display_pathbuf(requested)
    } else {
        display_pathbuf(canonical)
    }
}

pub(crate) fn current_timestamp_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_utc(secs)
}

/// Render a UNIX epoch in seconds as `YYYY-MM-DDTHH:MM:SSZ` using the
/// Gregorian proleptic calendar. No external date crate — we convert directly.
fn format_unix_utc(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let minute = (rem / 60) % 60;
    let second = rem % 60;

    // Calendar math (1970-01-01 is day 0). This implementation matches what
    // chrono does for sane inputs. Good enough for lockfile timestamps.
    let (year, month, day) = gregorian_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn gregorian_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    // Howard Hinnant's civil_from_days algorithm, adapted.
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn report(
    ctx: &output::Context,
    name: &str,
    display_root: &str,
    outcomes: &[Outcome],
) -> Result<()> {
    let created = outcomes.iter().filter(|o| o.action == Action::Created).count();
    let kept = outcomes.iter().filter(|o| o.action == Action::Kept).count();

    if ctx.is_json() {
        let payload = serde_json::json!({
            "ok": true,
            "command": "init",
            "project": name,
            "path": display_root,
            "created": created,
            "kept": kept,
            "outcomes": outcomes,
        });
        ctx.emit_json(&payload)?;
        return Ok(());
    }

    if ctx.is_quiet() {
        ctx.summary(&format!(
            "vibe init: {created} created, {kept} kept in `{display_root}`"
        ));
        return Ok(());
    }

    println!();
    ctx.summary(&format!(
        "Done. Project `{name}`: {created} file{} created, {kept} kept.",
        if created == 1 { "" } else { "s" }
    ));
    println!();
    println!("Next:");
    println!("  • edit spec/boot/00-core.md and spec/common/ as your project takes shape");
    println!("  • install packages with `vibe install <kind>:<name>` (e.g. flow:wal)");
    Ok(())
}

// ==== Templates ============================================================

const BOOT_90_USER_TEMPLATE: &str = r#"# User overrides

User-owned. `vibe install` / `vibe uninstall` never touch this file. Add any
project-specific conventions that should be read at session boot — coding
style, naming rules, deploy commands, anything the AI agent should know up
front and should not have to rediscover each session.
"#;

fn boot_00_core_template(project_name: &str) -> String {
    format!(
        r#"# Project boot snippet — `{project_name}`

User-owned. `vibe install` / `vibe uninstall` never touch this file.

## About this project

_TODO: one paragraph describing what `{project_name}` is and who it is for._

## Session boot sequence

Every AI session starts here. In order:
1. Read every file in `spec/boot/` in filename order.
2. Read `spec/WAL.md` — current project state (checkpoint, not history log).
3. Read the relevant PROP/FEAT documents under `spec/common/` and
   `spec/modules/` for the task at hand.
4. Only then begin work.

If `spec/WAL.md` is older than 24 hours, verify the state with the user before
doing destructive work.

## Memory layers

- **Head** (human): persistent but private.
- **WAL** (`spec/WAL.md`): volatile, rewritten each session, current state only.
- **Spec** (other files under `spec/`): stable decisions, addressable via
  `spec://<module>/<doc>#<section>` URIs.
- **Code** (`src/`, `tests/`): artefacts, regenerable.

Information flows top-down. When code changes first, reconcile up with a
Sync-from-Code proposal before rewriting code back to spec.

## Conflict resolution

Priority: **Human > Spec > Tests > Code.** When the AI believes the spec is
wrong, add a `<!-- REVIEW: … -->` marker, implement what the spec says, and
surface the disagreement in the end-of-session report.
"#
    )
}

fn wal_template(project_name: &str) -> String {
    let today = current_date_utc();
    format!(
        r#"# WAL — Project Continuation State
_Updated: {today}_

## Current phase

Project `{project_name}` — just initialized. No work in flight.

## Constraints (do not violate without discussion)

- (none yet — add as decisions are made)

## Done

- [x] Project initialized with `vibe init`.

## In progress

- (nothing)

## Next

- (fill in before starting the first real session)

## Known issues

- (none)

## Session context

- Start of next session: read this WAL, then `spec/boot/`, then the relevant
  PROP/FEAT under `spec/common/` or `spec/modules/`.
"#
    )
}

fn current_date_utc() -> String {
    let ts = current_timestamp_utc();
    ts.split('T').next().unwrap_or(&ts).to_string()
}

const ROOT_GITIGNORE_TEMPLATE: &str = r#"# vibevm cache (per-project, should never be committed)
/.vibe/

# OS / editor noise
.DS_Store
Thumbs.db
desktop.ini
.idea/
.vscode/
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gregorian_epoch_is_1970_01_01() {
        assert_eq!(gregorian_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn gregorian_one_day() {
        assert_eq!(gregorian_from_days(1), (1970, 1, 2));
    }

    #[test]
    fn gregorian_2026_04_16() {
        // Days from 1970-01-01 to 2026-04-16 =
        //   56 years * 365 + 14 leap days (1972, 76, 80, 84, 88, 92, 96, 2000,
        //   04, 08, 12, 16, 20, 24) + days in 2026 up to Apr 16 (Jan 31 + Feb 28 + Mar 31 + 16 = 106)
        // = 56*365 + 14 + 106 - 1 (because Jan 1 is day 0 of the year) = 20560 + 14 + 105 = 20679
        let (y, m, d) = gregorian_from_days(20_559);
        assert_eq!((y, m, d), (2026, 4, 16));
    }
}
