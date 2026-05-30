//! FT-113 — `dec drive show` — per-round narrative renderer for a feature drive.
//!
//! Operator-facing live or post-hoc view of what a `dec drive ship` run did.
//! Reads the orchestration store + `.dec/verify/{graph,result}/` files, groups
//! events into per-round narratives, and renders a compact human-readable timeline.

mod model;
mod reader;
mod render;
#[cfg(test)]
mod tests;

pub use model::{Dispatch, Outcome, Round, RoundState};
pub use reader::{DriveHistoryReader, ProductionReader, ReadError};
pub use render::{render_json, render_text, RenderOpts};

use std::path::Path;
use std::process::ExitCode;

pub struct ShowArgs {
    pub feature_id: String,
    pub bench: Option<String>,
    pub watch: bool,
    pub interval: Option<u64>,
    pub since: Option<u32>,
    pub format: String,
    pub all_drives: bool,
}

pub fn run(workdir: &Path, args: ShowArgs) -> ExitCode {
    // Validate format
    if args.format != "text" && args.format != "json" {
        eprintln!("dec drive show: format must be 'text' or 'json'");
        return ExitCode::from(2);
    }

    // Validate feature ID format
    if !args.feature_id.starts_with("FT-") {
        eprintln!("dec drive show: feature ID must start with 'FT-'");
        return ExitCode::from(2);
    }

    // Validate interval
    let interval = if let Some(i) = args.interval {
        if i < 1 || i > 60 {
            eprintln!("dec drive show: interval must be between 1 and 60 seconds");
            return ExitCode::from(2);
        }
        i
    } else {
        2 // default
    };

    // Check feature exists before proceeding
    if let Err(e) = check_feature_exists(workdir, &args.feature_id) {
        eprintln!("dec drive show: {e}");
        return ExitCode::from(1);
    }

    // Read rounds
    let reader = ProductionReader::new(workdir.to_path_buf());

    if args.watch {
        run_watch(&reader, &args, interval)
    } else {
        run_once(&reader, &args)
    }
}

// Helper to check if feature exists and provide better error message
fn check_feature_exists(workdir: &Path, feature_id: &str) -> Result<(), String> {
    let product_root = workdir.join(".product");
    if !product_root.exists() {
        return Err("product graph not found - run 'dec init' first".to_string());
    }

    let features_dir = product_root.join("features");
    if !features_dir.exists() {
        return Err(format!(
            "Unknown feature {}. Check `product feature list`.",
            feature_id
        ));
    }

    let feature_files: Vec<_> = std::fs::read_dir(&features_dir)
        .map_err(|e| format!("failed to read features dir: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(&format!("{}-", feature_id))
        })
        .collect();

    if feature_files.is_empty() {
        return Err(format!(
            "Unknown feature {}. Check `product feature list`.",
            feature_id
        ));
    }

    Ok(())
}

fn run_once(reader: &ProductionReader, args: &ShowArgs) -> ExitCode {
    let rounds = match reader.rounds_for_feature(&args.feature_id, args.bench.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("dec drive show: {e}");
            return ExitCode::from(1);
        }
    };

    let opts = RenderOpts {
        feature_id: args.feature_id.clone(),
        bench_id: args.bench.clone(),
        since: args.since,
    };

    let output = if args.format == "json" {
        render_json(&rounds)
    } else {
        render_text(&rounds, &opts)
    };

    print!("{output}");
    ExitCode::SUCCESS
}

fn run_watch(reader: &ProductionReader, args: &ShowArgs, interval: u64) -> ExitCode {
    use std::io::Write;
    use std::time::Duration;

    // Set up Ctrl-C handler
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();

    if let Err(e) = ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    }) {
        eprintln!("dec drive show: failed to set Ctrl-C handler: {e}");
        return ExitCode::from(1);
    }

    let opts = RenderOpts {
        feature_id: args.feature_id.clone(),
        bench_id: args.bench.clone(),
        since: args.since,
    };

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        // Read rounds
        let rounds = match reader.rounds_for_feature(&args.feature_id, args.bench.as_deref()) {
            Ok(r) => r,
            Err(e) => {
                // Don't fail on transient errors in watch mode
                eprintln!("dec drive show: error reading rounds: {e}");
                std::thread::sleep(Duration::from_secs(interval));
                continue;
            }
        };

        // Clear screen and render
        print!("\x1b[2J\x1b[H"); // ANSI clear screen
        std::io::stdout().flush().ok();

        let output = render_text(&rounds, &opts);
        print!("{output}");
        std::io::stdout().flush().ok();

        // Sleep
        std::thread::sleep(Duration::from_secs(interval));
    }

    println!("\ndec drive show: stopped");
    ExitCode::SUCCESS
}
