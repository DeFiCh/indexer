#![feature(error_generic_member_access)]
#![feature(vec_pop_if)]

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

use crate::lang::Result;

use args::Args;
use clap::Parser;
use lang::Error;
use tracing::error;

// TODO: Import grapher from ridx
fn run(_args: Args, _file_path: String) -> Result<()> {
    Ok(())
}

fn main_fallible() -> Result<()> {
    std::env::set_var("RUST_BACKTRACE", "1");
    tracing_subscriber::fmt::fmt().compact().init();

    let e = std::env::args_os();
    let mut e = e.collect::<Vec<_>>();

    let f = e
        .pop_if(|x| !x.to_str().unwrap().starts_with("-"))
        .ok_or("No file path provided")?;
    let f = f.into_string().map_err(|_| Error::from("err str"))?;

    let args = Args::try_parse_from(e)?;
    run(args, f)?;

    Ok(())
}

fn main() {
    let res = main_fallible();
    if let Err(e) = res {
        error!("{e}");
        let bt = std::error::request_ref::<std::backtrace::Backtrace>(&e);
        if let Some(bt) = bt {
            error!("{bt}");
        }
    }
}
