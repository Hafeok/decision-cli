//! Rendering functions for drive rounds (FT-113).

use super::model::{Outcome, Round};
use serde_json;

pub struct RenderOpts {
    pub feature_id: String,
    pub bench_id: Option<String>,
    pub since: Option<u32>,
}

pub fn render_text(rounds: &[Round], opts: &RenderOpts) -> String {
    if rounds.is_empty() {
        return render_empty_state(&opts.feature_id);
    }

    let mut output = String::new();

    // Filter rounds by --since if provided
    let rounds_to_render: Vec<&Round> = if let Some(since) = opts.since {
        rounds.iter().filter(|r| r.round_index >= since).collect()
    } else {
        rounds.iter().collect()
    };

    if rounds_to_render.is_empty() {
        return render_empty_state(&opts.feature_id);
    }

    // Render header
    output.push_str(&format!("{} — drive history\n\n", opts.feature_id));

    // Render each round
    for round in &rounds_to_render {
        output.push_str(&render_round(round));
        output.push('\n');
    }

    // Render summary
    if let Some(last) = rounds_to_render.last() {
        output.push_str(&render_summary(last, rounds.len()));
    }

    output
}

fn render_round(round: &Round) -> String {
    let mut output = String::new();

    // Round header with timestamp and elapsed
    let time_str = round.started_at.format("%H:%M:%S");
    let elapsed = format_duration(round.elapsed_since_round_zero);

    output.push_str(&format!(
        "Round {}  {}  [{}]\n",
        round.round_index, time_str, elapsed
    ));

    // State line
    output.push_str(&format!(
        "  state    {} · {} impl-open · {} vga-open · {} graphs\n",
        round.state.verdict, round.state.impl_open, round.state.vga_open, round.state.graph_count
    ));

    // Dispatch line
    output.push_str(&format!("  dispatch {}\n", round.dispatch.role));
    output.push_str(&format!("  ↳ session  {}\n", round.dispatch.session_iri));

    // Outcome lines
    output.push_str(&render_outcome(&round.outcome));

    output
}

fn render_outcome(outcome: &Outcome) -> String {
    match outcome {
        Outcome::VgaProduced {
            graph_id,
            steps,
            pass,
            fail,
        } => {
            format!(
                "  ↳ produced {} ({} steps, {} pass / {} fail)\n",
                graph_id, steps, pass, fail
            )
        }
        Outcome::ImplementerCommit {
            sha,
            addressed,
            remaining,
        } => {
            let mut s = format!("  ↳ commit   {}\n", sha);
            s.push_str(&format!("  ↳ addressed {} defects", addressed));
            if *remaining > 0 {
                s.push_str(&format!(" ({} remain)", remaining));
            }
            s.push('\n');
            s
        }
        Outcome::VerifierResults { pass, fail } => {
            format!("  ↳ results  {} pass / {} fail\n", pass, fail)
        }
        Outcome::Done => "  ↳ done\n".to_string(),
        Outcome::Stuck { reason } => {
            format!("  ↳ stuck: {}\n", reason)
        }
        Outcome::Partial { reason } => {
            format!("  ↳ partial: {}\n", reason)
        }
    }
}

fn render_summary(last_round: &Round, total_rounds: usize) -> String {
    let verdict = &last_round.state.verdict;
    let open = last_round.state.impl_open + last_round.state.vga_open;

    format!(
        "Current verdict:  {} · {} open defects · {} rounds\n",
        verdict.to_lowercase(),
        open,
        total_rounds
    )
}

fn render_empty_state(feature_id: &str) -> String {
    format!(
        "No drive history for {}. Run `dec drive ship {}` to start one.\n",
        feature_id, feature_id
    )
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs == 0 {
        "+0s".to_string()
    } else if secs < 60 {
        format!("+{}s", secs)
    } else if secs < 3600 {
        format!("+{}m{}s", secs / 60, secs % 60)
    } else {
        format!("+{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

pub fn render_json(rounds: &[Round]) -> String {
    serde_json::to_string_pretty(rounds)
        .unwrap_or_else(|e| format!("{{\"error\": \"failed to serialize: {}\"}}", e))
}
