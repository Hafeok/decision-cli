//! `dec session {show,list,log}` — session inspection (FT-012).

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;
use decision_cli::implement;
use decision_cli::session_inspect;

#[derive(Debug, Subcommand)]
pub enum SessionCmd {
    /// Show one Session by IRI (FT-012 / TC-008 step 5).
    Show(SessionShowArgs),
    /// List recent Sessions, oldest-first by start time (FT-012).
    List(SessionListArgs),
    /// Walk the PROV-O chain anchored on a Session (ADR-004 / FT-012).
    Log(SessionLogArgs),
}

#[derive(Debug, clap::Args)]
pub struct SessionShowArgs {
    /// Full IRI of the Session to display.
    pub iri: String,
}

#[derive(Debug, clap::Args)]
pub struct SessionListArgs {
    /// Maximum rows to return.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Rows to skip from the top of the ascending list.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Debug, clap::Args)]
pub struct SessionLogArgs {
    /// Full IRI of the Session whose PROV-O chain to walk.
    pub iri: String,
}

pub fn run(workdir: &Path, cmd: SessionCmd) -> ExitCode {
    match cmd {
        SessionCmd::Show(args) => run_show(workdir, &args.iri),
        SessionCmd::List(args) => run_list(workdir, args.limit, args.offset),
        SessionCmd::Log(args) => run_log(workdir, &args.iri),
    }
}

fn run_show(workdir: &Path, iri: &str) -> ExitCode {
    match implement::session_show(workdir, iri) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec session show: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn run_list(workdir: &Path, limit: usize, offset: usize) -> ExitCode {
    match session_inspect::list(workdir, limit, offset) {
        Ok(rows) => {
            print_session_rows(&rows);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec session list: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn print_session_rows(rows: &[session_inspect::SessionSummary]) {
    if rows.is_empty() {
        println!("(no sessions)");
    }
    for row in rows {
        let started = if row.started_at.is_empty() {
            "(unknown-time)"
        } else {
            row.started_at.as_str()
        };
        let feature = if row.feature_id.is_empty() {
            "(no-feature)"
        } else {
            row.feature_id.as_str()
        };
        let status = if row.status.is_empty() {
            "(pending)"
        } else {
            row.status.as_str()
        };
        println!(
            "{started}  feature={feature}  status={status}  iri={}",
            row.iri
        );
    }
}

fn run_log(workdir: &Path, iri: &str) -> ExitCode {
    match session_inspect::log(workdir, iri) {
        Ok(log) => {
            print!("{}", session_inspect::format_log(&log));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec session log: {err:#}");
            ExitCode::from(1)
        }
    }
}
