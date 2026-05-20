//! `dec events {since,tail}` — read-only event inspection (FT-012).

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;
use decision_cli::events as events_cmd;

#[derive(Debug, Subcommand)]
pub enum EventsCmd {
    /// Replay events with `oxi:seq >= <seq>` from the persisted store
    /// (FT-005).
    Since(EventsSinceArgs),
    /// Stream events live from the SSE endpoint of a running `dec`
    /// daemon (FT-004).
    Tail(EventsTailArgs),
}

#[derive(Debug, clap::Args)]
pub struct EventsSinceArgs {
    /// Inclusive lower bound on `oxi:seq`. Pass `0` to replay from the
    /// beginning of recorded history.
    pub seq: u64,
    /// Maximum rows to return; absent means unbounded.
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, clap::Args)]
pub struct EventsTailArgs {
    /// Override the SSE endpoint. Defaults to `DEC_EVENTS_URL` if set,
    /// otherwise `http://127.0.0.1:7878/events`.
    #[arg(long)]
    pub url: Option<String>,
}

pub fn run(workdir: &Path, cmd: EventsCmd) -> ExitCode {
    match cmd {
        EventsCmd::Since(args) => run_since(workdir, &args),
        EventsCmd::Tail(args) => run_tail(&args),
    }
}

fn run_since(workdir: &Path, args: &EventsSinceArgs) -> ExitCode {
    match events_cmd::since(workdir, args.seq, args.limit) {
        Ok(events) => {
            if events.is_empty() {
                println!("(no events with seq >= {})", args.seq);
            }
            for e in &events {
                println!("{}", events_cmd::format_event_line(e));
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec events since: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn run_tail(args: &EventsTailArgs) -> ExitCode {
    let url = args
        .url
        .clone()
        .or_else(|| std::env::var("DEC_EVENTS_URL").ok())
        .unwrap_or_else(|| events_cmd::DEFAULT_EVENTS_URL.to_string());
    match events_cmd::tail(&url) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("dec events tail: {err:#}");
            ExitCode::from(1)
        }
    }
}
