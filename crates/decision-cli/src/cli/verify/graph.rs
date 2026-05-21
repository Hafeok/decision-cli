//! `dec verify graph {new, list, show}` — clap adapter for the graph surface.

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::verify_graph_generate::{self, GenerateMode, GenerateRequest};
use decision_cli::verify_graph_list::{
    self, GraphListRequest, OutputFormat as GraphListFormat,
};
use decision_cli::verify_graph_new::{self, GraphNewRequest};
use decision_cli::verify_graph_show::{
    self, GraphShowRequest, OutputFormat as GraphShowFormat,
};

use super::exit_code_for;

#[derive(Debug, Subcommand)]
pub enum GraphCmd {
    /// Create a new VerificationGraph (FT-041).
    New(GraphNewArgs),
    /// List VerificationGraph artifacts (FT-042).
    List(GraphListArgs),
    /// Show a single VerificationGraph in detail (FT-043).
    Show(GraphShowArgs),
    /// Propose a graph for a feature in an environment (FT-049 / ADR-030).
    Generate(GraphGenerateArgs),
}

#[derive(Debug, clap::Args)]
pub struct GraphNewArgs {
    /// Caller-supplied id (e.g. VG-007). Omitted → mints the next free VG-NNN.
    #[arg(long)]
    pub id: Option<String>,
    /// Feature or TC the graph proves about (e.g. `FT-001`, `TC-013`).
    #[arg(long)]
    pub verifies: String,
    /// VerificationEnvironment the graph executes against (e.g. `ENV-001-ephemeral-cli`).
    #[arg(long)]
    pub environment: String,
}

#[derive(Debug, clap::Args)]
pub struct GraphListArgs {
    /// Optional filter — `FT-NNN` or `TC-NNN` reference.
    #[arg(long)]
    pub verifies: Option<String>,
    /// Optional environment filter (e.g. `ENV-001-ephemeral-cli`).
    #[arg(long)]
    pub environment: Option<String>,
    /// Output format. Defaults to `table`.
    #[arg(long, value_name = "FORMAT", default_value = "table")]
    pub format: String,
}

#[derive(Debug, clap::Args)]
pub struct GraphShowArgs {
    /// Identifier of the graph to show (e.g. `VG-001` or `VG-001-foo`).
    pub id: String,
    /// Output format. Defaults to `text`.
    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub format: String,
}

#[derive(Debug, clap::Args)]
pub struct GraphGenerateArgs {
    /// Feature id (e.g. `FT-049`).
    pub feature_id: String,
    /// Environment id (e.g. `ENV-001-ephemeral-cli`).
    #[arg(long)]
    pub environment: String,
    /// Persist the proposal without prompting.
    #[arg(long, conflicts_with = "print_only")]
    pub accept: bool,
    /// Print only; never persist.
    #[arg(long = "print-only", conflicts_with = "accept")]
    pub print_only: bool,
}

/// Build a [`GenerateRequest`] from clap args (CLI <-> MCP twin parity).
#[must_use]
pub fn graph_generate_request(args: &GraphGenerateArgs, workdir: &Path) -> GenerateRequest {
    let mode = if args.accept {
        GenerateMode::Accept
    } else if args.print_only {
        GenerateMode::PrintOnly
    } else {
        GenerateMode::Interactive
    };
    GenerateRequest {
        feature_id: args.feature_id.clone(),
        environment_id: args.environment.clone(),
        mode,
        workdir: Some(workdir.to_path_buf()),
        product_root: None,
    }
}

/// Convert graph-list clap args into the structured [`GraphListRequest`].
pub fn graph_list_request(
    args: &GraphListArgs,
    workdir: &Path,
) -> Result<GraphListRequest, HandlerError> {
    let format =
        GraphListFormat::parse(&args.format).ok_or_else(|| HandlerError::InvalidArgument {
            field: "format".to_string(),
            detail: format!(
                "format must be one of {{table, json}}; got {got:?}",
                got = args.format
            ),
        })?;
    Ok(GraphListRequest {
        verifies: args.verifies.clone(),
        environment: args.environment.clone(),
        format: Some(format),
        workdir: Some(workdir.to_path_buf()),
    })
}

/// Convert clap args into the structured `GraphNewRequest`. Exposed so
/// the TC-052 unit test can build the same request the binary does.
#[must_use]
pub fn graph_new_request(args: &GraphNewArgs, workdir: &Path) -> GraphNewRequest {
    GraphNewRequest {
        id: args.id.clone(),
        verifies: args.verifies.clone(),
        environment: args.environment.clone(),
        workdir: Some(workdir.to_path_buf()),
    }
}

/// Convert graph-show clap args into the structured [`GraphShowRequest`].
pub fn graph_show_request(
    args: &GraphShowArgs,
    workdir: &Path,
) -> Result<GraphShowRequest, HandlerError> {
    let format =
        GraphShowFormat::parse(&args.format).ok_or_else(|| HandlerError::InvalidArgument {
            field: "format".to_string(),
            detail: format!(
                "format must be one of {{text, json}}; got {got:?}",
                got = args.format
            ),
        })?;
    Ok(GraphShowRequest {
        id: args.id.clone(),
        format: Some(format),
        workdir: Some(workdir.to_path_buf()),
    })
}

pub(super) fn run(workdir: &Path, cmd: GraphCmd) -> ExitCode {
    match cmd {
        GraphCmd::New(args) => run_graph_new(workdir, args),
        GraphCmd::List(args) => run_graph_list(workdir, args),
        GraphCmd::Show(args) => run_graph_show(workdir, args),
        GraphCmd::Generate(args) => run_graph_generate(workdir, args),
    }
}

fn run_graph_generate(workdir: &Path, args: GraphGenerateArgs) -> ExitCode {
    let req = graph_generate_request(&args, workdir);
    match verify_graph_generate::run_generate(&req) {
        Ok(outcome) => {
            print_generate_outcome(&outcome);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify graph generate: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}

fn print_generate_outcome(outcome: &verify_graph_generate::GenerateResponse) {
    use decision_cli::verify_graph_generate::proposal::ProposalKind;
    match outcome.proposal.kind {
        ProposalKind::Match => {
            if let Some(m) = &outcome.proposal.match_payload {
                println!(
                    "{graph} already covers this feature in this environment; \
                     no new graph needed",
                    graph = m.graph_id
                );
            }
        }
        ProposalKind::New => match &outcome.persisted {
            Some(p) => {
                println!("Persisted VerificationGraph {id}", id = p.graph_id);
                println!("  Path: {}", p.graph_path.display());
                if p.coverage_report.uncovered.is_empty() {
                    println!("  Coverage: complete");
                } else {
                    println!(
                        "  Coverage gaps: {gaps:?}",
                        gaps = p.coverage_report.uncovered
                    );
                }
            }
            None => {
                println!(
                    "Proposed New graph (env={env}); re-run with --accept to persist.",
                    env = outcome
                        .proposal
                        .new
                        .as_ref()
                        .map(|n| n.environment.clone())
                        .unwrap_or_default()
                );
            }
        },
        ProposalKind::Gap => {
            if let Some(g) = &outcome.proposal.gap {
                println!("Gap — worker cannot produce a covering graph in this env.");
                println!("  Uncovered: {tcs:?}", tcs = g.uncovered_tcs);
                println!("  Reason: {r}", r = g.reason);
            }
        }
    }
}

fn run_graph_new(workdir: &Path, args: GraphNewArgs) -> ExitCode {
    let req = graph_new_request(&args, workdir);
    match verify_graph_new::run(&req) {
        Ok(outcome) => {
            println!("Created VerificationGraph {id}", id = outcome.id);
            println!("  Path: {}", outcome.path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify graph new: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}

fn run_graph_list(workdir: &Path, args: GraphListArgs) -> ExitCode {
    let req = match graph_list_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify graph list: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_graph_list::run(&req) {
        Ok(outcome) => {
            let format = req.format.unwrap_or_default();
            match format {
                GraphListFormat::Table => {
                    print!("{}", verify_graph_list::render_table(&outcome))
                }
                GraphListFormat::Json => {
                    println!("{}", verify_graph_list::render_json(&outcome))
                }
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify graph list: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}

fn run_graph_show(workdir: &Path, args: GraphShowArgs) -> ExitCode {
    let req = match graph_show_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify graph show: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_graph_show::run(&req) {
        Ok(outcome) => {
            let format = req.format.unwrap_or_default();
            match format {
                GraphShowFormat::Text => {
                    print!("{}", verify_graph_show::render_text(&outcome))
                }
                GraphShowFormat::Json => {
                    println!("{}", verify_graph_show::render_json(&outcome))
                }
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify graph show: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}
