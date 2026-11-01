mod assert;
mod github;
mod local;
mod process;
mod run;
mod sandbox;
mod scenarios;
mod seed;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};

use crate::assert::assert_run_set;
use crate::github::GitHub;
use crate::local::{run_local_scenario, LocalOptions};
use crate::run::{run_scenario, RunOptions, ScenarioResult};
use crate::sandbox::Sandbox;
use crate::scenarios::{scenarios, Scenario};

#[derive(Parser)]
#[command(
    name = "harness",
    version,
    about = "Run the continuous-release contract against a local fake GitHub or real GitHub Actions"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand)]
enum CliCommand {
    /// List the scenarios.
    List,
    /// Run scenarios locally (default) or live against GitHub Actions.
    Run(RunArgs),
    /// Recreate a sandbox repository from the template and move its base tag.
    Seed(SeedArgs),
}

#[derive(Args)]
struct RunArgs {
    /// Scenario names; with none, all scenarios run.
    scenarios: Vec<String>,
    /// Run every scenario (the default when no names are given).
    #[arg(long)]
    all: bool,
    /// Run against real GitHub Actions instead of the local fake GitHub.
    #[arg(long)]
    live: bool,
    /// Sandbox repository (live mode); defaults to SANDBOX_REPO or continuous-release/sandbox.
    #[arg(long)]
    repo: Option<String>,
    /// GitHub token (live mode); defaults to GITHUB_TOKEN.
    #[arg(long)]
    token: Option<String>,
    /// Select one shard of the scenario list, for example 0/4.
    #[arg(long)]
    shard: Option<String>,
    /// Leave releases, tags, and local repositories in place for inspection.
    #[arg(long)]
    keep: bool,
    /// Seconds to wait for a live workflow.
    #[arg(long, default_value_t = 900)]
    timeout: u64,
    /// Path to the cr binary (local mode); defaults to CR_BIN or cr.
    #[arg(long)]
    cr: Option<String>,
    /// Sandbox template directory; defaults to ./sandbox or the crate's sandbox.
    #[arg(long)]
    template: Option<PathBuf>,
}

#[derive(Args)]
struct SeedArgs {
    /// Repository to seed; defaults to SANDBOX_REPO or continuous-release/sandbox.
    #[arg(long)]
    repo: Option<String>,
    /// Sandbox template directory.
    #[arg(long)]
    template: Option<PathBuf>,
}

fn main() {
    if let Err(error) = execute() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn execute() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(CliCommand::List) {
        CliCommand::List => {
            for scenario in scenarios() {
                println!("{:<24} {}", scenario.name, scenario.description);
            }
            Ok(())
        }
        CliCommand::Run(args) => run_command(args),
        CliCommand::Seed(args) => {
            let repository = args
                .repo
                .or_else(|| std::env::var("SANDBOX_REPO").ok())
                .unwrap_or_else(|| "continuous-release/sandbox".into());
            let token = std::env::var("GITHUB_TOKEN")
                .context("GITHUB_TOKEN is required to seed a sandbox")?;
            let template = process::template_dir(args.template.as_deref())?;
            seed::seed(&repository, &token, &template)
        }
    }
}

fn run_command(args: RunArgs) -> Result<()> {
    let all = scenarios();
    let names = &args.scenarios;
    let missing: Vec<&String> = names
        .iter()
        .filter(|name| !all.iter().any(|scenario| scenario.name == name.as_str()))
        .collect();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|name| name.as_str()).collect();
        bail!("Unknown scenarios: {}", names.join(", "));
    }

    let selected: Vec<&Scenario> = if args.all || names.is_empty() {
        all.iter().collect()
    } else {
        all.iter()
            .filter(|scenario| names.iter().any(|name| name.as_str() == scenario.name))
            .collect()
    };

    let selected = if let Some(shard) = &args.shard {
        let (index, count) = parse_shard(shard)?;
        selected
            .into_iter()
            .enumerate()
            .filter(|(position, _)| position % count == index)
            .map(|(_, scenario)| scenario)
            .collect()
    } else {
        selected
    };

    let results = if args.live {
        run_live(&selected, &args)?
    } else {
        run_local(&selected, &args)?
    };

    println!("\n=== summary ===");
    for result in &results {
        let suffix = result
            .run_url
            .as_ref()
            .map(|url| format!(" {url}"))
            .unwrap_or_default();
        let detail = result
            .error
            .as_ref()
            .map(|error| format!("\n  {error}"))
            .unwrap_or_default();
        println!(
            "{} {}{}{}",
            if result.ok { "PASS" } else { "FAIL" },
            result.name,
            suffix,
            detail
        );
    }

    let failed = results.iter().filter(|result| !result.ok).count();
    println!("{}/{} passed", results.len() - failed, results.len());
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn run_live(selected: &[&Scenario], args: &RunArgs) -> Result<Vec<ScenarioResult>> {
    let repository = args
        .repo
        .clone()
        .or_else(|| std::env::var("SANDBOX_REPO").ok())
        .unwrap_or_else(|| "continuous-release/sandbox".into());
    let token = args
        .token
        .clone()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .context("GITHUB_TOKEN is required in live mode")?;

    let github = GitHub::new(token.clone(), repository.clone())?;
    let sandbox = Sandbox::new(&repository, &token)?;
    let options = RunOptions {
        keep: args.keep,
        timeout: Duration::from_secs(args.timeout),
    };

    let mut results = Vec::new();
    for scenario in selected {
        match run_scenario(scenario, &github, &sandbox, &options) {
            Ok(run) => results.push(ScenarioResult {
                name: scenario.name.to_string(),
                ok: true,
                error: None,
                run_url: Some(run.html_url),
            }),
            Err(error) => {
                let message = format!("{error:#}");
                eprintln!("FAILED: {}: {message}", scenario.name);
                results.push(ScenarioResult {
                    name: scenario.name.to_string(),
                    ok: false,
                    error: Some(message),
                    run_url: None,
                });
            }
        }
    }
    Ok(results)
}

fn run_local(selected: &[&Scenario], args: &RunArgs) -> Result<Vec<ScenarioResult>> {
    let cr_bin = args
        .cr
        .clone()
        .or_else(|| std::env::var("CR_BIN").ok())
        .unwrap_or_else(|| "cr".into());
    let status = Command::new(&cr_bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !matches!(status, Ok(status) if status.success()) {
        bail!(
            "Could not run {cr_bin}. Set CR_BIN to the toolkit binary, for example toolkit/target/release/cr."
        );
    }

    let template = process::template_dir(args.template.as_deref())?;
    let options = LocalOptions {
        keep: args.keep,
        cr_bin,
        template,
    };

    let mut results = Vec::new();
    for scenario in selected {
        let outcome = run_local_scenario(scenario, &options)
            .and_then(|set| assert_run_set(scenario, &set.initial, set.rerun.as_ref()).map(|_| ()));
        match outcome {
            Ok(()) => results.push(ScenarioResult {
                name: scenario.name.to_string(),
                ok: true,
                error: None,
                run_url: None,
            }),
            Err(error) => {
                let message = format!("{error:#}");
                eprintln!("FAILED: {}: {message}", scenario.name);
                results.push(ScenarioResult {
                    name: scenario.name.to_string(),
                    ok: false,
                    error: Some(message),
                    run_url: None,
                });
            }
        }
    }
    Ok(results)
}

fn parse_shard(value: &str) -> Result<(usize, usize)> {
    let (index, count) = value
        .split_once('/')
        .with_context(|| format!("--shard must look like 0/4, got {value}"))?;
    let index: usize = index
        .parse()
        .with_context(|| format!("--shard index is not a number: {index}"))?;
    let count: usize = count
        .parse()
        .with_context(|| format!("--shard count is not a number: {count}"))?;
    if count < 1 || index >= count {
        bail!("--shard {value} is out of range");
    }
    Ok((index, count))
}
