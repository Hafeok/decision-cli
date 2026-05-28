//! `dec implement` — slice-1 implementer dispatch (FT-011 + FT-013).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use decision_cli::implement::{self, ImplementArgs};

#[derive(Debug, clap::Args)]
pub struct ImplementCmdArgs {
    /// Feature id (e.g. `FT-007`).
    pub feature: String,
    /// Override the workspace directory the worker writes into.
    #[arg(long)]
    pub workspace: Option<PathBuf>,
    /// Override the path to product-cli's root (defaults to walking up
    /// from the working dir looking for `.product/`).
    #[arg(long)]
    pub product_root: Option<PathBuf>,
    /// Override the worker command (shell-quoted). Defaults to
    /// `code-writer` on $PATH, falling back to `python3 -m code_writer.main`.
    #[arg(long)]
    pub worker: Option<String>,
    /// Bundle assembly depth handed to `product context`.
    #[arg(long, default_value_t = 1)]
    pub bundle_depth: usize,
}

pub fn run(workdir: &Path, args: ImplementCmdArgs) -> ExitCode {
    let mut implement_args = ImplementArgs::new(args.feature);
    implement_args.workspace = args.workspace;
    implement_args.product_root = args.product_root;
    implement_args.worker_command = args.worker;
    implement_args.bundle_depth = args.bundle_depth;
    match implement::run(workdir, &implement_args) {
        Ok(outcome) => {
            println!("dec implement: success");
            println!("  Feature:        {}", implement_args.feature_id);
            println!("  Session:        {}", outcome.session_iri);
            println!("  Dispatch:       {}", outcome.dispatch_iri);
            println!("  CodeChange:     {}", outcome.code_change_iri);
            let short = &outcome.bundle_hash[..outcome.bundle_hash.len().min(12)];
            println!("  Bundle hash:    sha256:{short}…");
            println!("  Workspace:      {}", outcome.workspace_dir.display());
            println!(
                "  product graph:  {}",
                outcome.product_codechange_path.display()
            );
            for f in &outcome.files_written {
                println!("  wrote:          {}", f.display());
            }
            println!(
                "  Worker:         status={} turns={} latency={:.3}s",
                outcome.worker_status, outcome.turn_count, outcome.latency_seconds
            );
            if let Some(fin) = &outcome.finalize {
                match &fin.commit_sha {
                    Some(sha) => println!("  Commit:         {sha}"),
                    None => println!("  Commit:         (no working-tree changes)"),
                }
                if fin.status_transitioned {
                    println!(
                        "  Status:         {} → complete",
                        implement_args.feature_id
                    );
                } else {
                    println!(
                        "  Status:         {} → (not transitioned)",
                        implement_args.feature_id
                    );
                }
                for note in &fin.notes {
                    println!("  Note:           {note}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec implement failed: {err:#}");
            ExitCode::from(1)
        }
    }
}
