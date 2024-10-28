#![allow(dead_code)]

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None, propagate_version=true, next_line_help(true))]
pub struct Args {
    #[arg(long, default_value = "data/logs/")]
    pub graph_logs_path: String,
}
