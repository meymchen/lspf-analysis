mod cli;

use std::path::PathBuf;
use std::process;
use std::sync::{Arc, Mutex};
use std::thread::available_parallelism;

use clap::Parser;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;

use lspf_analysis::config::Settings;
use lspf_analysis::document::language_for;
use lspf_analysis::formats::Format;
use lspf_analysis::server;
use lspf_analysis_core::health::{FileHealth, HealthConfig, RepoHealth, file_health};
use lspf_analysis_core::{
    ConcurrentRunner, FilesData, FuncSpace, LANG, dump_root, get_from_ext, get_function_spaces,
    read_file_with_eol,
};

use cli::{Choice, Command, Metrics, Opts};

/// What `lspf-analysis metrics` reports for one file: the raw metrics tree
/// and the health scores derived from it, together, so a consumer never has
/// to recompute one from the other.
#[derive(Serialize)]
struct Report {
    metrics: FuncSpace,
    health: FileHealth,
}

struct MetricsConfig {
    language: Option<LANG>,
    output_format: Option<Format>,
    output: Option<PathBuf>,
    pretty: bool,
    health: HealthConfig,
    /// Every file's report, for the repository summary printed at the end.
    collected: Arc<Mutex<Vec<FileHealth>>>,
}

fn mk_globset(elems: Vec<String>) -> GlobSet {
    if elems.is_empty() {
        return GlobSet::empty();
    }
    let mut globset = GlobSetBuilder::new();
    elems.iter().filter(|e| !e.is_empty()).for_each(|e| {
        if let Ok(glob) = Glob::new(e) {
            globset.add(glob);
        }
    });
    globset.build().unwrap_or_else(|_| GlobSet::empty())
}

fn act_on_file(path: PathBuf, cfg: &MetricsConfig) -> std::io::Result<()> {
    let Some(source) = read_file_with_eol(&path)? else {
        return Ok(());
    };
    let Some(language) = cfg.language.or_else(|| language_for("", &path)) else {
        return Ok(());
    };
    let Some(space) = get_function_spaces(&language, source, &path) else {
        return Ok(());
    };
    let health = file_health(&space, &path, &cfg.health);

    if let Ok(mut collected) = cfg.collected.lock() {
        collected.push(health.clone());
    }

    match &cfg.output_format {
        Some(format) => {
            let report = Report {
                metrics: space,
                health,
            };
            format.dump_formats(report, path, cfg.output.as_ref(), cfg.pretty);
            Ok(())
        }
        None => {
            dump_root(&space)?;
            println!(
                "  quality: {:.0}% ({}), worst: {}",
                health.quality,
                health.grade,
                health
                    .worst()
                    .map(|function| format!(
                        "{} at {:.0}%",
                        function.display_name(),
                        function.quality
                    ))
                    .unwrap_or_else(|| "none".to_string()),
            );
            Ok(())
        }
    }
}

fn run_metrics(opts: Metrics) -> Result<(), String> {
    let output_is_dir = opts.output.as_ref().map(|p| p.is_dir()).unwrap_or(false);
    if opts.output.is_some() && !output_is_dir {
        return Err("the output parameter must be a directory".to_string());
    }

    let language = opts
        .language_type
        .as_deref()
        .filter(|typ| !typ.is_empty())
        .and_then(get_from_ext);
    if opts.language_type.is_some() && language.is_none() {
        return Err(format!(
            "unsupported language type {:?}",
            opts.language_type.unwrap_or_default()
        ));
    }

    let num_jobs = opts.num_jobs.unwrap_or_else(|| {
        available_parallelism()
            .map(|count| count.get())
            .unwrap_or(2)
    });

    let collected = Arc::new(Mutex::new(Vec::new()));
    let cfg = MetricsConfig {
        language,
        output_format: opts.output_format,
        output: opts.output,
        pretty: opts.pretty,
        health: HealthConfig::default(),
        collected: Arc::clone(&collected),
    };
    let files_data = FilesData {
        include: mk_globset(opts.include),
        exclude: mk_globset(opts.exclude),
        paths: opts.paths,
    };

    ConcurrentRunner::new(num_jobs, act_on_file)
        .run(cfg, files_data)
        .map_err(|error| format!("{error:?}"))?;

    let files = collected
        .lock()
        .map_err(|_| "a worker panicked while collecting reports".to_string())?;
    print_summary(&files, &HealthConfig::default());
    Ok(())
}

/// Prints the repository-level view: how the whole run scored, and the
/// functions most worth opening first.
fn print_summary(files: &[FileHealth], config: &HealthConfig) {
    let repo = RepoHealth::of(files.iter(), config);
    if repo.functions == 0 {
        println!("\nNo functions analyzed.");
        return;
    }
    println!(
        "\nRepository quality: {:.0}% ({}) over {} function{} in {} file{}",
        repo.quality,
        repo.grade,
        repo.functions,
        if repo.functions == 1 { "" } else { "s" },
        repo.files,
        if repo.files == 1 { "" } else { "s" },
    );
    println!(
        "  excellent {}, good {}, fair {}, poor {}",
        repo.excellent, repo.good, repo.fair, repo.poor
    );
    if !repo.worst.is_empty() {
        println!("  worst functions:");
        for (path, function) in &repo.worst {
            println!(
                "    {:>3.0}%  {}  {}:{}",
                function.quality,
                function.display_name(),
                path.display(),
                function.start_line
            );
        }
    }
}

/// Reads the settings a client sent in `initializationOptions`.
///
/// The binary cannot see them before `initialize` arrives, so the server
/// starts on defaults; the client's own `didChangeConfiguration` replaces
/// them, and this is only the environment escape hatch for a client that
/// sends neither.
fn initial_settings() -> Settings {
    let Ok(raw) = std::env::var("LSPF_ANALYSIS_SETTINGS") else {
        return Settings::default();
    };
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) => Settings::from_value(&value).unwrap_or_default(),
        Err(error) => {
            tracing::warn!(%error, "ignoring unreadable LSPF_ANALYSIS_SETTINGS");
            Settings::default()
        }
    }
}

async fn run_serve(choice: Choice) -> lspf::Result<lspf::Outcome> {
    let build = || server(lspf::OsFileProvider::new(), initial_settings());
    match choice {
        Choice::Stdio => lspf::stdio(build()).serve().await,
        Choice::Tcp(addr) => {
            lspf::tcp(build(), addr.as_str())
                .on_bound(|bound| tracing::info!(%bound, "listening for one TCP client"))
                .serve()
                .await
        }
        Choice::WebSocket(addr) => {
            lspf::websocket(build(), addr.as_str())
                .on_bound(|bound| tracing::info!(%bound, "listening for one WebSocket client"))
                .serve()
                .await
        }
    }
}

#[tokio::main]
async fn main() {
    // Logs go to stderr: stdout carries the LSP wire protocol and nothing else.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    match Opts::parse().command {
        Command::Serve(serve) => match run_serve(serve.transport.choice()).await {
            Ok(outcome) => process::exit(outcome.code()),
            Err(error) => {
                eprintln!("lspf-analysis: {error}");
                process::exit(1);
            }
        },
        Command::Metrics(metrics) => {
            if let Err(error) = run_metrics(metrics) {
                eprintln!("lspf-analysis: {error}");
                process::exit(1);
            }
        }
    }
}
