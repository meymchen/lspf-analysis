//! The command line: serve a connection, or run an analysis over some
//! paths. Each analysis gets its own subcommand; `metrics` is the first.

use std::path::PathBuf;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, Parser, Subcommand};

use lspf_analysis::formats::Format;

#[derive(Parser, Debug)]
#[clap(
    name = "lspf-analysis",
    version,
    about = "Analyze source code, as LSP diagnostics or from the command line."
)]
pub struct Opts {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Serve one LSP connection.
    Serve(Serve),
    /// Compute metrics and health scores for files or directories.
    Metrics(Metrics),
}

/// Which transport carries the connection. Exactly one applies; with none
/// given the server speaks stdio, which is what editors launch.
#[derive(Args, Debug)]
#[group(multiple = false)]
pub struct Transport {
    /// Serve over stdin and stdout. The default.
    #[clap(long)]
    pub stdio: bool,
    /// Serve one TCP client on this address, e.g. 127.0.0.1:9257.
    #[clap(long, value_name = "ADDR")]
    pub tcp: Option<String>,
    /// Serve one WebSocket client on this address, e.g. 127.0.0.1:9258.
    #[clap(long, value_name = "ADDR")]
    pub ws: Option<String>,
}

#[derive(Args, Debug)]
pub struct Serve {
    #[clap(flatten)]
    pub transport: Transport,
}

#[derive(Args, Debug)]
pub struct Metrics {
    /// Files or directories to analyze.
    #[clap(long, short, value_parser, required = true)]
    pub paths: Vec<PathBuf>,
    /// Glob to include files.
    #[clap(long, short = 'I', num_args(0..))]
    pub include: Vec<String>,
    /// Glob to exclude files.
    #[clap(long, short = 'X', num_args(0..))]
    pub exclude: Vec<String>,
    /// Number of jobs.
    #[clap(long, short = 'j')]
    pub num_jobs: Option<usize>,
    /// Language type, given as a file extension, e.g. `rs`.
    #[clap(long, short)]
    pub language_type: Option<String>,
    /// Serialize each file's report in this format instead of printing a
    /// metrics tree.
    #[clap(long, short = 'O', value_parser = PossibleValuesParser::new(Format::all())
        .map(|s| s.parse::<Format>().unwrap()))]
    pub output_format: Option<Format>,
    /// Write serialized output into this directory instead of stdout.
    #[clap(long, short, value_parser)]
    pub output: Option<PathBuf>,
    /// Pretty-print the serialized output.
    #[clap(long = "pr")]
    pub pretty: bool,
}

impl Transport {
    /// Returns the chosen transport.
    pub fn choice(&self) -> Choice {
        if let Some(addr) = &self.tcp {
            Choice::Tcp(addr.clone())
        } else if let Some(addr) = &self.ws {
            Choice::WebSocket(addr.clone())
        } else {
            Choice::Stdio
        }
    }
}

/// One of the three ways the native binary can be reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Stdio,
    Tcp(String),
    WebSocket(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_cli_definition_is_valid() {
        Opts::command().debug_assert();
    }

    fn transport_of(args: &[&str]) -> Choice {
        match Opts::parse_from(args).command {
            Command::Serve(serve) => serve.transport.choice(),
            other => panic!("expected serve, got {other:?}"),
        }
    }

    #[test]
    fn serving_defaults_to_stdio() {
        assert_eq!(transport_of(&["lspf-analysis", "serve"]), Choice::Stdio);
        assert_eq!(
            transport_of(&["lspf-analysis", "serve", "--stdio"]),
            Choice::Stdio
        );
    }

    #[test]
    fn each_transport_flag_selects_its_transport() {
        assert_eq!(
            transport_of(&["lspf-analysis", "serve", "--tcp", "127.0.0.1:9257"]),
            Choice::Tcp("127.0.0.1:9257".into())
        );
        assert_eq!(
            transport_of(&["lspf-analysis", "serve", "--ws", "127.0.0.1:9258"]),
            Choice::WebSocket("127.0.0.1:9258".into())
        );
    }

    #[test]
    fn transports_are_mutually_exclusive() {
        assert!(
            Opts::try_parse_from([
                "lspf-analysis",
                "serve",
                "--tcp",
                "127.0.0.1:1",
                "--ws",
                "127.0.0.1:2",
            ])
            .is_err()
        );
    }

    #[test]
    fn metrics_requires_at_least_one_path() {
        assert!(Opts::try_parse_from(["lspf-analysis", "metrics"]).is_err());
        assert!(Opts::try_parse_from(["lspf-analysis", "metrics", "--paths", "src"]).is_ok());
    }
}
