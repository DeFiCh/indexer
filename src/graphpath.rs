use crate::{
    db::SqliteBlockStore,
    lang::{OptionExt, Result},
    models::TxType,
};
use anyhow::Context;
use clap::Parser;
use tracing::{debug, error, info};

#[derive(Parser, Debug)]
pub struct GraphPathArgs {
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(long, default_value = "data/graph.bin")]
    pub graph_data_path: String,
    #[arg(long, default_value = "data/graph.meta.bin")]
    pub graph_meta_path: String,
    /// Source address
    #[arg(long, short = 'a')]
    pub src_addr: String,
    /// Dest address
    #[arg(long, short = 'd')]
    pub dest_addr: String,
}

pub fn run(args: &GraphPathArgs) -> Result<()> {
    debug!("args: {:?}", args);

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;

    let sql_store = SqliteBlockStore::new_v2(Some(&args.sqlite_path))?;
    let (g, node_index_map) = load_graph(&args.graph_meta_path, &args.graph_data_path)?;

    let src_addr = &args.src_addr;
    let dest_addr = &args.dest_addr;

    let src_index = node_index_map.get(src_addr).context("src_index")?;
    let dest_index = node_index_map.get(dest_addr).context("dest_index")?;

    info!("finding path..");

    let paths = petgraph::algo::astar(
        &g,
        *src_index,
        |finish| finish == *dest_index,
        |_e| 1,
        |_finish| 0,
    );

    debug!("{:?}", paths);

    match paths {
        Some((_cost, path)) => {
            for (i, node_idx) in path.windows(2).enumerate() {
                let src_node = g.node_weight(node_idx[0]).context("node_weight")?;
                let dest_node = g.node_weight(node_idx[1]).context("node_weight")?;

                let edge = g.find_edge(node_idx[0], node_idx[1]).context("find_edge")?;
                let tx_id = g.edge_weight(edge).context("edge_weight")?;

                let tx = sql_store.get_tx_data(&tx_id)?.ok_or_err()?;
                let tx_type = TxType::from_display(&tx.tx_type.as_str());

                info!(
                    "[{}] {}: {} -> {} (tx: {})",
                    i, tx_type, src_node, dest_node, tx_id,
                );
            }
        }
        None => {
            info!("no path found");
        }
    }

    info!("complete");
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
        bincode::deserialize_from(f).context("meta bincode err")?;

    info!("loading graph data from {}..", data_path);
    let f = std::fs::File::open(data_path)?;
    let g: petgraph::Graph<String, String> =
        bincode::deserialize_from(f).context("g bincode err")?;

    info!(
        "loaded graph with {} nodes and {} edges",
        g.node_count(),
        g.edge_count()
    );
    Ok((g, node_index_map))
}
