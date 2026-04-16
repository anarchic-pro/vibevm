//! Output helpers. The CLI has two modes: human-readable (default) and JSON
//! (`--json`). `--quiet` collapses human-readable output to a single summary
//! line. See `VIBEVM-SPEC.md` §9.3.

use console::Style;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Human,
    HumanQuiet,
    Json,
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
}

impl Context {
    pub fn from_flags(quiet: bool, json: bool) -> Self {
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
        }
    }

    pub fn is_json(&self) -> bool {
        self.mode == Mode::Json
    }

    pub fn is_quiet(&self) -> bool {
        self.mode == Mode::HumanQuiet
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
                let payload = serde_json::json!({
                    "ok": false,
                    "error": format!("{err:#}"),
                });
                eprintln!("{payload}");
            }
        }
    }

    pub fn emit_json<T: Serialize>(&self, value: &T) -> anyhow::Result<()> {
        if !self.is_json() {
            return Ok(());
        }
        let rendered = serde_json::to_string_pretty(value)?;
        println!("{rendered}");
        Ok(())
    }
}
