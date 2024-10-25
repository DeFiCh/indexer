#![feature(error_generic_member_access)]

#[path = "../args.rs"]
mod args;
#[path = "../db.rs"]
mod db;
#[path = "../dfiutils.rs"]
mod dfiutils;
#[path = "../lang.rs"]
mod lang;
#[path = "../models.rs"]
mod models;

use std::error::request_ref;

use crate::lang::Result;

use args::{get_args, verbosity_to_level, Args};
use db::SqliteBlockStore;
use tracing::error;

// TODO: Import grapher from ridx
fn run(args: &Args) -> Result<()> {
    let sql_store = SqliteBlockStore::new(Some(&args.sqlite_path))?;

    Ok(())
}

fn main_fallible() -> Result<()> {
    std::env::set_var("RUST_BACKTRACE", "1");
    let args = get_args();
    tracing_subscriber::fmt::fmt()
        .with_max_level(verbosity_to_level(args.verbosity, Some(2)))
        .compact()
        .init();
    run(args)?;
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
