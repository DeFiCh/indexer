use clap::{Parser, Subcommand};
use std::sync::LazyLock;
use tracing::Level;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None, propagate_version=true, next_line_help(true))]
pub struct Args {
    /// Can be called multiple times to increase level. (0-4).
    ///
    /// 0: Error
    /// 1: Warn
    /// 2: Info
    /// 3: Debug
    /// 4: Trace
    ///
    /// Minimum might be pulled higher.
    #[arg(global = true, short, long, action = clap::ArgAction::Count, verbatim_doc_comment)]
    pub verbosity: u8,
    #[command(subcommand)]
    pub command: Cmd,
}
#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Index from cli sqlite db
    #[command(name = "index")]
    Index(crate::sqliteindexer::IndexArgs),
    /// Reduce dot graph files
    #[command(name = "dotreduce")]
    DotReduce {
        #[arg(long = "in")]
        in_file: String,
    },
    /// Analyze ICX addr usages
    #[command(name = "icx1")]
    ICXAnalyze(crate::icxanalyzer::ICXAnalyzeArgs),
    /// Build full graph
    #[command(name = "graph")]
    Graph(crate::grapher::GrapherArgs),
    /// Load and explore full graph
    #[command(name = "graphexp")]
    GraphExp(crate::graphexplorer::GraphExpArgs),
    /// Load the full graph, condense it and output dot files
    #[command(name = "graphdot")]
    GraphDot(crate::graphdot::GraphDotArgs),
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
