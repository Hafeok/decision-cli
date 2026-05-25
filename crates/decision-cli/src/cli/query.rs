//! `dec query template {list,show}` — QueryTemplate catalog inspection
//! (FT-075 / ADR-043).
//!
//! Read-only subcommands; slice-1 ships inspection only. Execution
//! against runtime focal artifacts is the job of audit tooling which
//! consumes the catalog via `core::queries::full_chain`.

#![allow(missing_docs)]

use std::path::Path;
use std::process::ExitCode;

use clap::{Args, Subcommand};

use decision_cli::core::queries::{
    fetch_query_template, list_query_templates, QueryTemplate, QueryTemplateError,
};
use decision_cli::core::store::open_orchestration_store;

#[derive(Debug, Subcommand)]
pub enum QueryCmd {
    /// QueryTemplate catalog (FT-075 / ADR-043).
    #[command(subcommand)]
    Template(TemplateCmd),
}

#[derive(Debug, Subcommand)]
pub enum TemplateCmd {
    /// List every registered QueryTemplate (id, version, language).
    List,
    /// Print one QueryTemplate's full spec + version + provenance.
    Show(ShowArgs),
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// QueryTemplate identifier — accepts `qt:full-chain-backward-v1`,
    /// the full IRI, or the bare local-name (`full-chain-backward-v1`).
    pub id: String,
}

pub fn run(workdir: &Path, cmd: QueryCmd) -> ExitCode {
    match cmd {
        QueryCmd::Template(t) => run_template(workdir, t),
    }
}

fn run_template(workdir: &Path, cmd: TemplateCmd) -> ExitCode {
    match cmd {
        TemplateCmd::List => run_list(workdir),
        TemplateCmd::Show(args) => run_show(workdir, &args.id),
    }
}

fn run_list(workdir: &Path) -> ExitCode {
    let store = match open_orchestration_store(workdir) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("dec query template list: {err:#}");
            return ExitCode::from(1);
        }
    };
    let templates = match list_query_templates(&store) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("dec query template list: {err}");
            return ExitCode::from(1);
        }
    };
    if templates.is_empty() {
        println!("(no query templates registered)");
        return ExitCode::SUCCESS;
    }
    for qt in templates {
        println!(
            "{id}  version={ver}  language={lang}  iri={iri}",
            id = qt.id,
            ver = qt.version,
            lang = qt.language,
            iri = qt.iri
        );
    }
    ExitCode::SUCCESS
}

fn run_show(workdir: &Path, id: &str) -> ExitCode {
    let store = match open_orchestration_store(workdir) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("dec query template show: {err:#}");
            return ExitCode::from(1);
        }
    };
    match fetch_query_template(&store, id) {
        Ok(qt) => {
            print_template(&qt);
            ExitCode::SUCCESS
        }
        Err(QueryTemplateError::TemplateNotFound { id }) => {
            eprintln!("dec query template show: template not found: {id}");
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("dec query template show: {err}");
            ExitCode::from(1)
        }
    }
}

fn print_template(qt: &QueryTemplate) {
    println!("id:       {}", qt.id);
    println!("iri:      {}", qt.iri);
    println!("version:  {}", qt.version);
    println!("language: {}", qt.language);
    println!("spec:");
    for line in qt.spec.lines() {
        println!("  {line}");
    }
}
