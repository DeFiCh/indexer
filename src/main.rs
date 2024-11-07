#![feature(error_generic_member_access)]

mod args;
mod db;
mod dfiutils;
mod dotreducer;
mod gpath;
mod graphdot;
mod grapher;
mod graphutils;
mod graphwalker;
mod icxanalyzer;
mod icxseq;
mod lang;
mod logparse;
mod models;
mod spath;
mod sqliteindexer;

use crate::lang::Result;
use args::{get_args, verbosity_to_level, Cmd};
use std::error::request_ref;
use tracing::error;

fn main_fallible() -> Result<()> {
    std::env::set_var("RUST_BACKTRACE", "1");
    let args = get_args();
    let emit_ansi = atty::is(atty::Stream::Stdout);

    tracing_subscriber::fmt::fmt()
        .with_max_level(verbosity_to_level(args.verbosity, Some(2)))
        .with_ansi(emit_ansi)
        .compact()
        .init();

    match &args.command {
        Cmd::Index(a) => sqliteindexer::run(a)?,
        Cmd::DotReduce { in_file } => {
            dotreducer::run(in_file)?;
        }
        Cmd::IcxAnalyze(a) => icxanalyzer::run(a)?,
        Cmd::IcxSequence(a) => icxseq::run(a)?,
        Cmd::Graph(a) => grapher::run(a)?,
        Cmd::GraphWalk(a) => graphwalker::run(a)?,
        Cmd::GraphDot(a) => graphdot::run(a)?,
        Cmd::ShortestPath(a) => spath::run(a)?,
        Cmd::GraphPath(a) => gpath::run(a)?,
        Cmd::LogParseCheck(a) => logparse::run(a)?,
    }
    Ok(())
}

fn main() {
    let res = main_fallible();
    if let Err(e) = res {
        error!("{e}");
        let bt = request_ref::<std::backtrace::Backtrace>(&e);
        if let Some(bt) = bt {
            error!("{bt}");
        }
    }
}
