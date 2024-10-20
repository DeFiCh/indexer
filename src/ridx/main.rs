#![feature(error_generic_member_access)]

#[path = "../lang.rs"]
mod lang;
#[path = "../models.rs"]
mod models;
#[path = "../utils.rs"]
mod utils;

mod blockindexer;
mod grapher;
mod txindexer;

use crate::lang::Result;

use std::env;
use tracing::info;

fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "1");

    tracing_subscriber::fmt().compact().with_ansi(false).init();

    let mode = 3;
    match mode {
        0 => blockindexer::check_db_index()?,
        1 => blockindexer::index_from_cli()?,
        2 => txindexer::index_tx_data()?,
        3 => grapher::graph_it()?,
        _ => info!("error"),
    };

    Ok(())
}
