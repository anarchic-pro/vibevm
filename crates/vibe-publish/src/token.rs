//! Publish-token loading.
//!
//! Order of precedence:
//! 1. Explicit value (`Token::from_explicit`).
//! 2. `VIBEVM_PUBLISH_TOKEN` environment variable.
//! 3. `~/.vibevm/git.publish.token` file (whitespace trimmed).
//!
//! Tokens are surface-secret; never logged at any level. The
//! [`Token`] type wraps the string and `Display`s as `***` to make
//! accidental logging visible at code-review time.

use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::PublishError;

/// Where the loaded token came from. Not the value — that stays inside
/// the `Token`. Useful in CLI output ("loaded token from
/// $VIBEVM_PUBLISH_TOKEN") and error attribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    Explicit,
    EnvVar(&'static str),
    File(PathBuf),
}

/// Wraps a publish token string so it never accidentally lands in a log.
#[derive(Clone)]
pub struct Token {
    value: String,
    source: TokenSource,
}

impl Token {
    pub fn from_explicit(value: impl Into<String>) -> Self {
        Token {
            value: value.into(),
            source: TokenSource::Explicit,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn source(&self) -> &TokenSource {
        &self.source
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Token")
            .field("value", &"***")
            .field("source", &self.source)
            .finish()
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

const ENV_VAR: &str = "VIBEVM_PUBLISH_TOKEN";

/// Load a token using the standard precedence. `host` is purely for
/// error attribution — different hosts can in principle use different
/// tokens, but today there is one well-known location.
pub fn load_token(host: &str) -> Result<Token, PublishError> {
    if let Ok(value) = std::env::var(ENV_VAR) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(Token {
                value: trimmed.to_string(),
                source: TokenSource::EnvVar(ENV_VAR),
            });
        }
    }

    if let Some(path) = default_token_path()
        && path.exists()
    {
        let raw = fs::read_to_string(&path).map_err(|e| PublishError::Io {
            path: path.clone(),
            message: format!("reading token: {e}"),
        })?;
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(Token {
                value: trimmed.to_string(),
                source: TokenSource::File(path),
            });
        }
    }

    Err(PublishError::AuthMissing {
        host: host.to_string(),
    })
}

/// Default location for the publish token file —
/// `<home>/.vibevm/git.publish.token`.
pub fn default_token_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".vibevm").join("git.publish.token"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_value_lands() {
        let t = Token::from_explicit("abc123");
        assert_eq!(t.value(), "abc123");
        assert!(matches!(t.source(), TokenSource::Explicit));
    }

    #[test]
    fn debug_redacts_value() {
        let t = Token::from_explicit("super-secret-12345");
        let s = format!("{t:?}");
        assert!(!s.contains("super-secret-12345"));
        assert!(s.contains("***"));
    }

    #[test]
    fn display_redacts_value() {
        let t = Token::from_explicit("super-secret-12345");
        let s = format!("{t}");
        assert!(!s.contains("super-secret-12345"));
        assert_eq!(s, "***");
    }

    // Tests that mutate process-wide environment variables would need
    // `unsafe` (Rust 2024 marks `std::env::set_var` / `remove_var` as
    // unsafe due to global-state cross-thread hazards), and this crate
    // has `#![forbid(unsafe_code)]`. Skip env-mutation tests; the
    // construction path is exercised by the explicit-value test plus
    // the redaction tests above. Live env / file behaviour gets a
    // smoke-test pass during real publish runs.
}
