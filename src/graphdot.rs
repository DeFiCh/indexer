use crate::lang::Result;
use crate::{db::SqliteBlockStore, graphutils};
use clap::Parser;
use tracing::{debug, info};

#[derive(Parser, Debug)]
pub struct GraphDotArgs {
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(long, default_value = "data/graph.bin")]
    pub graph_data_path: String,
    #[arg(long, default_value = "data/graph.meta.bin")]
    pub graph_meta_path: String,
    #[arg(long, default_value = "data/graph.dot")]
    pub graph_out_path: String,
    #[arg(long, default_value = "data/graph.acyc.dot")]
    pub graph_out_acyclic_path: String,
}

pub fn run(args: &GraphDotArgs) -> Result<()> {
    debug!("args: {:?}", args);

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;
    let user_sig = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(unix)]
    signal_hook::flag::register(
        signal_hook::consts::SIGUSR1,
        std::sync::Arc::clone(&user_sig),
    )?;

    let _sql_store = SqliteBlockStore::new_v2(Some(&args.sqlite_path))?;
    let (g, _node_index_map) =
        graphutils::load_graph(&args.graph_meta_path, &args.graph_data_path)?;
    let gx = petgraph::algo::condensation(g, true);

    info!(
        "condensed: {} nodes and {} edges",
        gx.node_count(),
        gx.edge_count()
    );

    // TODO: condense out
    // TODO: condense acyclic out

    Ok(())
}
