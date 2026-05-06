//! `vibe-index serve <data-dir>` — boot the HTTP server.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

use crate::error::{Error, Result};
use crate::index::Index;
use crate::server::{AppState, ServerLock, build_app};

#[derive(Debug, Parser)]
#[command(about = "Run the HTTP server.")]
pub struct Args {
    pub data_dir: PathBuf,

    /// Address to bind. Default: `127.0.0.1:8412` (local-only).
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1:8412")]
    pub bind: SocketAddr,

    /// File containing one bearer token per line. Slice 5 ignores
    /// this; slice 6 wires the auth layer.
    #[arg(long, value_name = "FILE")]
    pub auth_tokens_file: Option<PathBuf>,

    /// Refuse every mutating endpoint regardless of auth (slice 5
    /// has no mutating endpoints anyway, so the flag effectively
    /// pins the read-only posture).
    #[arg(long)]
    pub read_only: bool,

    /// After every successful mutation, `git add -A && git commit &&
    /// git push` in the data directory. Slice 5 stub.
    #[arg(long)]
    pub auto_commit_push: bool,
}

pub fn run(args: Args) -> Result<()> {
    let _ = args.auto_commit_push; // parked until slice 9.
    let _ = args.auth_tokens_file; // parked until slice 6.

    let index = Index::load_from(&args.data_dir).map_err(|e| match e {
        Error::Io { .. } | Error::Malformed(_) => Error::InvalidInput(format!(
            "data-dir `{}` does not look like an initialised index. \
             Run `vibe-index init` first.",
            args.data_dir.display()
        )),
        other => other,
    })?;

    let lock = ServerLock::try_acquire(&args.data_dir)?;

    let state = AppState::new(args.data_dir.clone(), args.read_only, index);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::Io {
            path: args.data_dir.clone(),
            message: format!("could not build tokio runtime: {e}"),
        })?;

    runtime.block_on(async move {
        let app = build_app(state);
        let listener = tokio::net::TcpListener::bind(args.bind).await.map_err(|e| {
            Error::InvalidInput(format!("could not bind {}: {e}", args.bind))
        })?;

        eprintln!(
            "vibe-index serving `{}` at http://{} (read-only={}, pid={})",
            args.data_dir.display(),
            args.bind,
            args.read_only,
            std::process::id(),
        );

        let server = axum::serve(listener, app);
        tokio::select! {
            r = server => r.map_err(|e| Error::Io {
                path: args.data_dir.clone(),
                message: format!("server: {e}"),
            }),
            _ = tokio::signal::ctrl_c() => {
                eprintln!("vibe-index: SIGINT received, shutting down");
                Ok(())
            }
        }
    })?;

    drop(lock);
    Ok(())
}
