//! Cluster-aware `dec session show` renderers (FT-161 / FT-146).
//!
//! Two entry points exposed to the parent module:
//!
//! - [`render_cluster_cell`] — `urn:dec:cluster-session:*` per-cell view
//!   (capability, status, usage source, timing, token breakdown,
//!   parent activity IRI).
//! - [`render_cluster_dispatch`] — `urn:dec:cluster-dispatch:*` aggregate
//!   view: header (feature, task type, outcome, timing) plus per-cell
//!   token + cost table summed at the foot. Pricing uses the
//!   `dec:Capability` cost rates loaded from the same store.
//!
//! Both render byte-stable output (lexicographic cell ordering, MAX over
//! repeated token quads — matching the read pattern in
//! `core::graph::session::apply_solution`).

use std::collections::BTreeMap;
use std::fmt::Write;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, FixedOffset};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::DEC_NS;

/// Render a `urn:dec:cluster-session:<task-type>/<feature>/<cell>` IRI.
pub(super) fn render_cluster_cell(store: &Store, session_iri: &str) -> Result<String> {
    let row = fetch_cluster_cell(store, session_iri)?;
    let row = row.ok_or_else(|| {
        anyhow!("session-show: no cluster cell session with IRI <{session_iri}>")
    })?;

    let cluster = row.parent_cluster.as_deref().unwrap_or("(unknown)");
    let cap = row.capability.as_deref().unwrap_or("(none)");
    let status = row.status.as_deref().unwrap_or("(unknown)");
    let src = row.usage_source.as_deref().unwrap_or("(unreported)");
    let start = row.started_at.as_deref().unwrap_or("(unknown)");
    let end = row.ended_at.as_deref().unwrap_or("(unknown)");
    let dur = duration_seconds(row.started_at.as_deref(), row.ended_at.as_deref())
        .map(|s| format!("{s:.2}s"))
        .unwrap_or_else(|| "—".to_string());

    let total_input = row.input_base + row.input_cache_write + row.input_cache_hit;
    let mut out = String::new();
    writeln!(out, "Cell session   {session_iri}").unwrap();
    writeln!(out, "Cluster        {cluster}").unwrap();
    writeln!(out, "Capability     {cap}").unwrap();
    writeln!(out, "Status         {status}").unwrap();
    writeln!(out, "Usage source   {src}").unwrap();
    writeln!(out, "Started        {start}").unwrap();
    writeln!(out, "Ended          {end}").unwrap();
    writeln!(out, "Duration       {dur}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "Tokens:").unwrap();
    writeln!(out, "  input_tokens_base            {:>10}", row.input_base).unwrap();
    writeln!(
        out,
        "  input_tokens_cache_write     {:>10}",
        row.input_cache_write
    )
    .unwrap();
    writeln!(
        out,
        "  input_tokens_cache_hit       {:>10}",
        row.input_cache_hit
    )
    .unwrap();
    writeln!(out, "  output_tokens                {:>10}", row.output).unwrap();
    writeln!(out, "  total input                  {total_input:>10}").unwrap();
    Ok(out)
}

/// Render a `urn:dec:cluster-dispatch:<task-type>/<feature>` IRI as
/// aggregate report: header + per-cell table + total.
pub(super) fn render_cluster_dispatch(store: &Store, cluster_iri: &str) -> Result<String> {
    let header = fetch_cluster_header(store, cluster_iri)?;
    let mut cells = fetch_cluster_children(store, cluster_iri)?;
    cells.sort_by(|a, b| a.iri.cmp(&b.iri));

    let costs = load_capability_costs(store).unwrap_or_default();

    let feature = header.as_ref().and_then(|h| h.feature.clone());
    let task_type = header.as_ref().and_then(|h| h.task_type.clone());
    let outcome = header
        .as_ref()
        .and_then(|h| h.latest_outcome.clone())
        .unwrap_or_else(|| "(unknown)".into());
    let outcome_count = header.as_ref().map(|h| h.outcome_count).unwrap_or(0);
    let started = header
        .as_ref()
        .and_then(|h| h.started_at.clone())
        .unwrap_or_else(|| "(unknown)".into());
    let ended = header
        .as_ref()
        .and_then(|h| h.ended_at.clone())
        .unwrap_or_else(|| "(unknown)".into());
    let dur = duration_seconds(
        header.as_ref().and_then(|h| h.started_at.as_deref()),
        header.as_ref().and_then(|h| h.ended_at.as_deref()),
    )
    .map(|s| format!("{s:.2}s"))
    .unwrap_or_else(|| "—".to_string());

    let mut out = String::new();
    writeln!(out, "Cluster        {cluster_iri}").unwrap();
    writeln!(
        out,
        "Feature        {}",
        feature.as_deref().unwrap_or("(unknown)")
    )
    .unwrap();
    writeln!(
        out,
        "Task type      {}",
        task_type.as_deref().unwrap_or("(unknown)")
    )
    .unwrap();
    if outcome_count > 1 {
        writeln!(out, "Outcome        {outcome} ({outcome_count} runs aggregated)").unwrap();
    } else {
        writeln!(out, "Outcome        {outcome}").unwrap();
    }
    writeln!(out, "Started        {started}").unwrap();
    writeln!(out, "Ended          {ended}").unwrap();
    writeln!(out, "Duration       {dur}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "Cells ({}):", cells.len()).unwrap();
    if cells.is_empty() {
        writeln!(
            out,
            "  (no cells linked via prov:wasInformedBy — cluster recorded but children absent)"
        )
        .unwrap();
        return Ok(out);
    }

    let header_line = format!(
        "  {:<22} {:<12} {:<16} {:>8} {:>5} {:>5} {:>8} {:>10}",
        "cell", "status", "src", "base", "cw", "ch", "output", "cost"
    );
    let sep = "  ".to_string() + &"─".repeat(22) + " " + &"─".repeat(12) + " " + &"─".repeat(16)
        + " "
        + &"─".repeat(8)
        + " "
        + &"─".repeat(5)
        + " "
        + &"─".repeat(5)
        + " "
        + &"─".repeat(8)
        + " "
        + &"─".repeat(10);
    writeln!(out, "{header_line}").unwrap();
    writeln!(out, "{sep}").unwrap();

    let mut totals: BTreeMap<String, (u64, u64, u64, u64, f64, bool)> = BTreeMap::new();
    let mut unpriced_cells = 0usize;
    for cell in &cells {
        let name = cell.short_name();
        let status = cell.status.as_deref().unwrap_or("?");
        let src = cell.usage_source.as_deref().unwrap_or("?");

        let (cost_str, currency) = match cell
            .capability
            .as_deref()
            .and_then(|c| costs.get(c))
        {
            Some(rates) => {
                let c = compute_cell_cost(cell, rates);
                (format!("{}{:.4}", currency_symbol(&rates.currency), c), Some(rates.currency.clone()))
            }
            None => {
                unpriced_cells += 1;
                ("—".to_string(), None)
            }
        };

        writeln!(
            out,
            "  {:<22} {:<12} {:<16} {:>8} {:>5} {:>5} {:>8} {:>10}",
            name, status, src, cell.input_base, cell.input_cache_write, cell.input_cache_hit, cell.output, cost_str
        )
        .unwrap();

        let key = currency.clone().unwrap_or_else(|| "(unpriced)".into());
        let entry = totals.entry(key).or_insert((0, 0, 0, 0, 0.0, false));
        entry.0 += cell.input_base;
        entry.1 += cell.input_cache_write;
        entry.2 += cell.input_cache_hit;
        entry.3 += cell.output;
        if let Some(rates) = cell.capability.as_deref().and_then(|c| costs.get(c)) {
            entry.4 += compute_cell_cost(cell, rates);
            entry.5 = true;
        }
    }
    writeln!(out, "{sep}").unwrap();
    for (currency, (b, cw, ch, o, cost, priced)) in &totals {
        // Don't emit a TOTAL (unpriced) row when its only contributors are
        // zero-token cells (mechanical) — that's noise, not signal.
        let zero_only = *b == 0 && *cw == 0 && *ch == 0 && *o == 0;
        if currency == "(unpriced)" && zero_only {
            continue;
        }
        let label = if *currency == "(unpriced)" {
            "TOTAL (unpriced)".to_string()
        } else {
            format!("TOTAL {currency}")
        };
        let cost_render = if *priced {
            format!("{}{:.4}", currency_symbol(currency), cost)
        } else {
            "—".to_string()
        };
        writeln!(
            out,
            "  {:<22} {:<12} {:<16} {:>8} {:>5} {:>5} {:>8} {:>10}",
            label, "", "", b, cw, ch, o, cost_render
        )
        .unwrap();
    }
    // Annotate only when an unpriced cell contributed non-zero tokens
    // (mechanical zero-cells are not user-actionable noise).
    let nonzero_unpriced = cells.iter().any(|c| {
        let unpriced = c.capability.as_deref().map(|cap| !costs.contains_key(cap)).unwrap_or(true);
        unpriced && (c.input_base + c.input_cache_write + c.input_cache_hit + c.output) > 0
    });
    if unpriced_cells > 0 && nonzero_unpriced {
        writeln!(
            out,
            "  ({unpriced_cells} cell(s) unpriced — capability cost rates not in store)"
        )
        .unwrap();
    }
    Ok(out)
}

// --------------------------------------------------------------------
// Row types + fetchers
// --------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct CellRow {
    iri: String,
    capability: Option<String>,
    status: Option<String>,
    usage_source: Option<String>,
    started_at: Option<String>,
    ended_at: Option<String>,
    parent_cluster: Option<String>,
    input_base: u64,
    input_cache_write: u64,
    input_cache_hit: u64,
    output: u64,
}

impl CellRow {
    /// Short cell name (last path segment of the cell IRI).
    fn short_name(&self) -> &str {
        self.iri.rsplit('/').next().unwrap_or(self.iri.as_str())
    }
}

#[derive(Debug, Clone, Default)]
struct ClusterHeader {
    feature: Option<String>,
    task_type: Option<String>,
    latest_outcome: Option<String>,
    outcome_count: usize,
    started_at: Option<String>,
    ended_at: Option<String>,
}

fn fetch_cluster_cell(store: &Store, session_iri: &str) -> Result<Option<CellRow>> {
    let q = format!(
        "PREFIX dec: <{DEC_NS}>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?cap ?status ?src ?started ?ended ?parent
       (MAX(?base) AS ?baseMax) (MAX(?cw) AS ?cwMax)
       (MAX(?ch) AS ?chMax) (MAX(?out) AS ?outMax)
WHERE {{
  GRAPH ?g {{
    OPTIONAL {{ <{session_iri}> dec:capability ?cap }}
    OPTIONAL {{ <{session_iri}> dec:cellStatus ?status }}
    OPTIONAL {{ <{session_iri}> dec:usageSource ?src }}
    OPTIONAL {{ <{session_iri}> prov:startedAtTime ?started }}
    OPTIONAL {{ <{session_iri}> prov:endedAtTime ?ended }}
    OPTIONAL {{ <{session_iri}> prov:wasInformedBy ?parent }}
    OPTIONAL {{ <{session_iri}> dec:input_tokens_base ?base }}
    OPTIONAL {{ <{session_iri}> dec:input_tokens_cache_write ?cw }}
    OPTIONAL {{ <{session_iri}> dec:input_tokens_cache_hit ?ch }}
    OPTIONAL {{ <{session_iri}> dec:output_tokens ?out }}
  }}
}}
GROUP BY ?cap ?status ?src ?started ?ended ?parent",
    );
    let results = store.query(&q).context("cluster-cell SPARQL")?;
    let QueryResults::Solutions(mut sols) = results else {
        return Err(anyhow!("cluster-cell: unexpected SPARQL shape"));
    };
    let Some(sol_res) = sols.next() else {
        return Ok(None);
    };
    let sol = sol_res.context("cluster-cell SPARQL solution")?;
    let mut row = CellRow {
        iri: session_iri.to_string(),
        ..Default::default()
    };
    row.capability = iri_value(&sol, "cap");
    row.status = literal_value(&sol, "status");
    row.usage_source = literal_value(&sol, "src");
    row.started_at = literal_value(&sol, "started");
    row.ended_at = literal_value(&sol, "ended");
    row.parent_cluster = iri_value(&sol, "parent");
    row.input_base = literal_u64(&sol, "baseMax");
    row.input_cache_write = literal_u64(&sol, "cwMax");
    row.input_cache_hit = literal_u64(&sol, "chMax");
    row.output = literal_u64(&sol, "outMax");

    // Treat "all fields empty" as not-found so the caller can render
    // the standard "no Session with IRI" error.
    if row.capability.is_none()
        && row.status.is_none()
        && row.parent_cluster.is_none()
        && row.input_base == 0
        && row.output == 0
    {
        return Ok(None);
    }
    Ok(Some(row))
}

fn fetch_cluster_header(store: &Store, cluster_iri: &str) -> Result<Option<ClusterHeader>> {
    // Multiple outcome triples may exist on the same cluster IRI across
    // re-runs. Pick the most-recent by ?ended ordering; count the total
    // so the renderer can note "(N runs aggregated)".
    let q = format!(
        "PREFIX dec: <{DEC_NS}>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?feature ?taskType ?outcome ?started ?ended
WHERE {{
  GRAPH ?g {{
    OPTIONAL {{ <{cluster_iri}> dec:featureId ?feature }}
    OPTIONAL {{ <{cluster_iri}> dec:taskType ?taskType }}
    OPTIONAL {{ <{cluster_iri}> dec:clusterOutcome ?outcome }}
    OPTIONAL {{ <{cluster_iri}> prov:startedAtTime ?started }}
    OPTIONAL {{ <{cluster_iri}> prov:endedAtTime ?ended }}
  }}
}}
ORDER BY DESC(?ended)",
    );
    let results = store.query(&q).context("cluster-header SPARQL")?;
    let QueryResults::Solutions(sols) = results else {
        return Err(anyhow!("cluster-header: unexpected SPARQL shape"));
    };
    let mut header = ClusterHeader::default();
    let mut seen_outcomes = std::collections::BTreeSet::<String>::new();
    let mut any = false;
    for sol_res in sols {
        let sol = sol_res.context("cluster-header SPARQL solution")?;
        any = true;
        if header.feature.is_none() {
            header.feature = literal_value(&sol, "feature");
        }
        if header.task_type.is_none() {
            header.task_type = literal_value(&sol, "taskType");
        }
        if header.started_at.is_none() {
            header.started_at = literal_value(&sol, "started");
        }
        if let Some(outcome) = literal_value(&sol, "outcome") {
            // First row is highest ended_at (most recent).
            if header.latest_outcome.is_none() {
                header.latest_outcome = Some(outcome.clone());
                header.ended_at = literal_value(&sol, "ended");
            }
            seen_outcomes.insert(outcome);
        }
    }
    if !any {
        return Ok(None);
    }
    header.outcome_count = seen_outcomes.len();
    Ok(Some(header))
}

fn fetch_cluster_children(store: &Store, cluster_iri: &str) -> Result<Vec<CellRow>> {
    // FT-161 dedup: group by ?cell only, SAMPLE the varying fields.
    // Re-dispatching the same cluster IRI accumulates triples per cell;
    // we want one row per cell with MAX-of-tokens (latest attempt) and
    // SAMPLE-of-status / src (any one — operator-visible choice but
    // deterministic per store contents).
    let q = format!(
        "PREFIX dec: <{DEC_NS}>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?cell
       (SAMPLE(?cap)    AS ?capS)
       (SAMPLE(?status) AS ?statusS)
       (SAMPLE(?src)    AS ?srcS)
       (MAX(?base) AS ?baseMax) (MAX(?cw) AS ?cwMax)
       (MAX(?ch) AS ?chMax) (MAX(?out) AS ?outMax)
WHERE {{
  GRAPH ?g {{
    ?cell prov:wasInformedBy <{cluster_iri}> .
    OPTIONAL {{ ?cell dec:capability ?cap }}
    OPTIONAL {{ ?cell dec:cellStatus ?status }}
    OPTIONAL {{ ?cell dec:usageSource ?src }}
    OPTIONAL {{ ?cell dec:input_tokens_base ?base }}
    OPTIONAL {{ ?cell dec:input_tokens_cache_write ?cw }}
    OPTIONAL {{ ?cell dec:input_tokens_cache_hit ?ch }}
    OPTIONAL {{ ?cell dec:output_tokens ?out }}
  }}
}}
GROUP BY ?cell",
    );
    let results = store.query(&q).context("cluster-children SPARQL")?;
    let QueryResults::Solutions(sols) = results else {
        return Err(anyhow!("cluster-children: unexpected SPARQL shape"));
    };
    let mut out = Vec::new();
    for sol_res in sols {
        let sol = sol_res.context("cluster-children SPARQL solution")?;
        let Some(iri) = iri_value(&sol, "cell") else {
            continue;
        };
        let row = CellRow {
            iri,
            capability: iri_value(&sol, "capS"),
            status: literal_value(&sol, "statusS"),
            usage_source: literal_value(&sol, "srcS"),
            input_base: literal_u64(&sol, "baseMax"),
            input_cache_write: literal_u64(&sol, "cwMax"),
            input_cache_hit: literal_u64(&sol, "chMax"),
            output: literal_u64(&sol, "outMax"),
            ..Default::default()
        };
        out.push(row);
    }
    Ok(out)
}

// --------------------------------------------------------------------
// Capability cost map loader
// --------------------------------------------------------------------

/// Minimal cost-rate record used by FT-161's renderer. Keeps the cluster
/// renderer decoupled from `core::graph::session::CapabilityCostRates`
/// (which carries the full FT-057 SessionView shape).
#[derive(Debug, Clone, Default)]
pub(super) struct CostRates {
    input_per_m: f64,
    output_per_m: f64,
    cache_write_per_m: Option<f64>,
    cache_hit_per_m: Option<f64>,
    currency: String,
}

fn load_capability_costs(store: &Store) -> Result<BTreeMap<String, CostRates>> {
    let q = format!(
        "PREFIX dec: <{DEC_NS}>
SELECT ?cap ?in ?out ?cw ?ch ?cur
WHERE {{
  GRAPH ?g {{
    ?cap dec:cost_input_per_m ?in ;
         dec:cost_output_per_m ?out ;
         dec:cost_currency ?cur .
    OPTIONAL {{ ?cap dec:cost_cache_write_5m ?cw }}
    OPTIONAL {{ ?cap dec:cost_cache_hit_per_m ?ch }}
  }}
}}",
    );
    let results = store.query(&q).context("capability-cost SPARQL")?;
    let QueryResults::Solutions(sols) = results else {
        return Err(anyhow!("capability-cost: unexpected SPARQL shape"));
    };
    let mut out = BTreeMap::new();
    for sol_res in sols {
        let sol = sol_res.context("capability-cost SPARQL solution")?;
        let Some(cap) = iri_value(&sol, "cap") else {
            continue;
        };
        let Some(input) = literal_value(&sol, "in").and_then(|v| v.parse().ok()) else {
            continue;
        };
        let Some(output) = literal_value(&sol, "out").and_then(|v| v.parse().ok()) else {
            continue;
        };
        let currency = literal_value(&sol, "cur").unwrap_or_else(|| "USD".into());
        out.insert(
            cap,
            CostRates {
                input_per_m: input,
                output_per_m: output,
                cache_write_per_m: literal_value(&sol, "cw").and_then(|v| v.parse().ok()),
                cache_hit_per_m: literal_value(&sol, "ch").and_then(|v| v.parse().ok()),
                currency,
            },
        );
    }
    Ok(out)
}

fn compute_cell_cost(cell: &CellRow, rates: &CostRates) -> f64 {
    let base = cell.input_base as f64 * rates.input_per_m / 1_000_000.0;
    let cw = cell.input_cache_write as f64
        * rates.cache_write_per_m.unwrap_or(rates.input_per_m)
        / 1_000_000.0;
    let ch = cell.input_cache_hit as f64
        * rates.cache_hit_per_m.unwrap_or(rates.input_per_m)
        / 1_000_000.0;
    let out = cell.output as f64 * rates.output_per_m / 1_000_000.0;
    base + cw + ch + out
}

fn currency_symbol(currency: &str) -> &'static str {
    match currency {
        "EUR" => "€",
        "USD" => "$",
        _ => "",
    }
}

// --------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------

fn iri_value(sol: &oxigraph::sparql::QuerySolution, name: &str) -> Option<String> {
    match sol.get(name)? {
        oxigraph::model::Term::NamedNode(n) => Some(n.as_str().to_string()),
        _ => None,
    }
}

fn literal_value(sol: &oxigraph::sparql::QuerySolution, name: &str) -> Option<String> {
    match sol.get(name)? {
        oxigraph::model::Term::Literal(lit) => Some(lit.value().to_string()),
        _ => None,
    }
}

fn literal_u64(sol: &oxigraph::sparql::QuerySolution, name: &str) -> u64 {
    literal_value(sol, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn duration_seconds(start: Option<&str>, end: Option<&str>) -> Option<f64> {
    let s = DateTime::<FixedOffset>::parse_from_rfc3339(start?).ok()?;
    let e = DateTime::<FixedOffset>::parse_from_rfc3339(end?).ok()?;
    Some((e - s).num_milliseconds() as f64 / 1000.0)
}
