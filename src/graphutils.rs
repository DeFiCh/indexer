use crate::lang::Result;
use anyhow::Context;
use tracing::info;

pub fn load_graph(
    meta_path: &str,
    data_path: &str,
) -> Result<(
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
