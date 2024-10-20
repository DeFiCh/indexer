#![feature(error_generic_member_access)]

#[path = "../db.rs"]
mod db;
#[path = "../dfiutils.rs"]
mod dfiutils;
#[path = "../lang.rs"]
mod lang;
#[path = "../models.rs"]
mod models;

#[path = "../args.rs"]
mod args;

mod blockindexer;
mod grapher;
mod txindexer;

use crate::lang::Result;

use std::env;
use clap::Parser;
use tracing::info;

fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "1");

    tracing_subscriber::fmt().compact().with_ansi(false).init();

    let args = args::Args::parse();

    let mode = 3;
    match mode {
        0 => blockindexer::check_db_index()?,
        1 => blockindexer::index_from_cli()?,
        2 => txindexer::index_tx_data()?,
        3 => grapher::graph_it(args)?,
        _ => info!("error"),
    };

    Ok(())
}
