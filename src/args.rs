#![allow(dead_code)]

use clap::{Parser, Subcommand};
use std::sync::LazyLock;
use tracing::Level;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None, propagate_version=true, next_line_help(true))]
pub struct Args {
    #[arg(short, long, action = clap::ArgAction::Count, verbatim_doc_comment)]
    /// Can be called multiple times to increase level. (0-4).
    ///
    /// 0: Error
    /// 1: Warn
    /// 2: Info
    /// 3: Debug
    /// 4: Trace
    ///
    /// Minimum might be pulled higher.
    pub verbosity: u8,
    #[command(subcommand)]
    pub cmd: Option<Commands>,
    #[arg(long, default_value = "defi-cli")]
    pub defi_cli_path: String,
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(long, default_value = "data/debug.log")]
    pub defid_log_path: String,
    #[arg(long, default_value = "data/logs")]
    pub graph_logs_path: String,
    #[arg(long, default_value = "claim_tx")]
    pub defid_log_matcher: String,
    #[arg(short = 's', long, default_value_t = 0)]
    pub start_height: i64,
    #[arg(short = 'e', long, default_value_t = 2_000_000)]
    pub end_height: i64,
    #[arg(long, default_value_t = true)]
    pub enable_graph_table: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Default,
}

pub fn verbosity_to_level(verbosity: u8, min: Option<u8>) -> Level {
    let m = min.unwrap_or(0);
    let v = if verbosity < m { m } else { verbosity };
    match v {
        0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        4 => Level::TRACE,
        _ => Level::TRACE,
    }
}

pub fn get_args() -> &'static Args {
    static ARGS: LazyLock<Args> = LazyLock::new(Args::parse);
    &ARGS
}
