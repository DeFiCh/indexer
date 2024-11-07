use crate::lang::Result;
use clap::{Parser, Subcommand};
use std::{io::BufRead, sync::LazyLock};
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
    #[command(name = "cindex")]
    CliIndex(crate::cliindexer::CliIndexArgs),
    /// Index / transform / migrate from src sqlite db to dest db
    #[command(name = "sindex")]
    SqliteIndex(crate::sqliteindex::SqliteIndexArgs),
    /// Reduce dot graph files
    #[command(name = "dotreduce")]
    DotReduce {
        #[arg(long = "in")]
        in_file: String,
    },
    /// Analyze ICX claims and every address involved in the way
    /// up until the swap of the claims
    #[command(name = "icx1")]
    IcxAnalyze1(crate::icx1::IcxAnalyzeArgs),
    /// ICX analysis 2
    #[command(name = "icx2")]
    IcxAnalyze2(crate::icx2::IcxAnalyze2Args),
    /// Output the full ICX sequence chain
    #[command(name = "icxseq")]
    IcxSequence(crate::icxseq::IcxSequenceArgs),
    /// Construct the full graph and output it to a file
    /// so the graph can loaded in memory and reused directly.
    #[command(name = "gbuild")]
    Graph(crate::graphbuild::GrapherArgs),
    /// Load and explore full graph
    #[command(name = "gwalk")]
    GraphWalk(crate::graphwalk::GraphWalkArgs),
    /// Load the full graph, condense it and output dot files
    #[command(name = "gdot")]
    GraphDot(crate::graphdot::GraphDotArgs),
    /// Find all paths with exclusions
    #[command(name = "gpath")]
    GraphPath(crate::gpath::GraphPathArgs),
    /// Find shortest path between 2 addresses or a list of given addresses
    #[command(name = "spath")]
    ShortestPath(crate::spath::ShortestPathArgs),
    /// Parse logs that will be ingested and merge into index during indexing
    /// and check for errors
    #[command(name = "logparsecheck")]
    LogParseCheck(crate::logparse::LogParseArgs),
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

pub fn process_list_args_with_file_paths(list: &[String]) -> Result<Vec<String>> {
    let mut r_list: Vec<String> = Vec::with_capacity(list.len());
    for x in list.iter() {
        if let Ok(f) = std::fs::File::open(x) {
            let mut r = std::io::BufReader::new(f);
            let mut buf = String::new();
            while r.read_line(&mut buf)? != 0 {
                let line = buf.trim();
                if line.is_empty() {
                    continue;
                }
                r_list.push(line.to_string());
                buf.clear();
            }
        } else {
            r_list.push(x.clone());
        }
    }
    Ok(r_list)
}
