use crate::db::SqliteBlockStore;
use crate::lang::Result;
use clap::Parser;
use tracing::{debug, error, info, trace};

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

    let sql_store = SqliteBlockStore::new_v2(Some(&args.sqlite_path))?;
    let (g, node_index_map) = load_graph(&args.graph_meta_path, &args.graph_data_path)?;
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

pub fn load_graph(
    meta_path: &str,
    data_path: &str,
) -> crate::lang::Result<(
    petgraph::Graph<String, String>,
    std::collections::HashMap<String, petgraph::graph::NodeIndex>,
)> {
    info!("loading graph metadata from {}..", meta_path);
    let f = std::fs::File::open(meta_path)?;
    let node_index_map: std::collections::HashMap<String, petgraph::graph::NodeIndex> =
        bincode::deserialize_from(f).map_err(|e| {
            error!("{:?}", e);
            "bincode err"
        })?;

    info!("loading graph data from {}..", data_path);
    let f = std::fs::File::open(data_path)?;
    let g: petgraph::Graph<String, String> = bincode::deserialize_from(f).map_err(|e| {
        error!("{:?}", e);
        "bincode err"
    })?;

    info!(
        "loaded graph with {} nodes and {} edges",
        g.node_count(),
        g.edge_count()
    );
    Ok((g, node_index_map))
}
