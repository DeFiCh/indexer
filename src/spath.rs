// WIP file. Remove on finish
#![allow(unused_variables)]

use std::collections::HashSet;

use crate::args::process_list_args_with_file_paths;
use crate::graphutils;
use crate::{
    db::SqliteBlockStore,
    lang::{OptionExt, Result},
    models::TxType,
};
use anyhow::Context;
use clap::Parser;
use tracing::{debug, info};

#[derive(Parser, Debug)]
pub struct ShortestPathArgs {
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(long, default_value = "data/graph.bin")]
    pub graph_data_path: String,
    #[arg(long, default_value = "data/graph.meta.bin")]
    pub graph_meta_path: String,
    /// Source address
    #[arg(
        long,
        short = 'a',
        required = true,
        use_value_delimiter = true,
        value_delimiter = ','
    )]
    pub src: Vec<String>,
    /// Dest address
    #[arg(
        long,
        short = 'd',
        required = true,
        use_value_delimiter = true,
        value_delimiter = ','
    )]
    pub dest: Vec<String>,
    /// Ignore list to ignore paths with given addresses
    #[arg(long, short = 'd', use_value_delimiter = true, value_delimiter = ',')]
    pub ignore: Vec<String>,
}

pub fn run(args: &ShortestPathArgs) -> Result<()> {
    debug!("args: {:?}", args);

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;

    let (src_addrs, dest_addrs, ignore_addrs) = (
        process_list_args_with_file_paths(&args.src)?,
        process_list_args_with_file_paths(&args.dest)?,
        process_list_args_with_file_paths(&args.ignore)?
            .into_iter()
            .collect::<HashSet<_>>(),
    );

    let sql_store = SqliteBlockStore::new_v2(Some(&args.sqlite_path))?;
    let (g, node_index_map) = graphutils::load_graph(&args.graph_meta_path, &args.graph_data_path)?;

    if ignore_addrs.is_empty() {
        // Without ignore list is much easier, since we can use A* to only go after the single path.
        path_find_astar_fixed_cost(src_addrs, dest_addrs, quit, node_index_map, g, sql_store)?;
    } else {
        // This is going to be more work, as we have no choice but to evaluate more paths.
        // Alternatively, implement a custom A* that attaches a high cost and skip nodes on seeing
        // the ignore list, but still stops search at a certain level.
        path_find_with_ignore(
            src_addrs,
            dest_addrs,
            ignore_addrs,
            quit,
            node_index_map,
            g,
            sql_store,
        )?;
    }

    info!("complete");
    Ok(())
}

fn path_find_with_ignore(
    src_addrs: Vec<String>,
    dest_addrs: Vec<String>,
    ignore_addrs: HashSet<String>,
    quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    node_index_map: std::collections::HashMap<String, petgraph::prelude::NodeIndex>,
    g: petgraph::Graph<String, String>,
    sql_store: SqliteBlockStore,
) -> Result<()> {
    for src in src_addrs.iter() {
        for dest in dest_addrs.iter() {
            if quit.load(std::sync::atomic::Ordering::Relaxed) {
                info!("int: early exit");
                return Err("interrupted".into());
            }
            info!("finding path: {} -> {}", src, dest);

            let src_index = node_index_map.get(src).context("src_index")?;
            let dest_index = node_index_map.get(dest).context("dest_index")?;

            todo!("unimplemented");
        }
    }
    Ok(())
}

fn path_find_astar_fixed_cost(
    src_addrs: Vec<String>,
    dest_addrs: Vec<String>,
    quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    node_index_map: std::collections::HashMap<String, petgraph::prelude::NodeIndex>,
    g: petgraph::Graph<String, String>,
    sql_store: SqliteBlockStore,
) -> Result<()> {
    for src in src_addrs.iter() {
        for dest in dest_addrs.iter() {
            if quit.load(std::sync::atomic::Ordering::Relaxed) {
                info!("int: early exit");
                return Err("interrupted".into());
            }
            info!("finding path: {} -> {}", src, dest);

            let src_index = node_index_map.get(src);
            if src_index.is_none() {
                info!("src not found: {}", src);
                continue;
            }
            let dest_index = node_index_map.get(dest);
            if dest_index.is_none() {
                info!("dest not found: {}", dest);
                continue;
            }

            let src_index = src_index.unwrap();
            let dest_index = dest_index.unwrap();

            let paths = petgraph::algo::astar(
                &g,
                *src_index,
                |node| node == *dest_index,
                |_edge| 1,
                |_node| 0,
            );

            debug!("{:?}", paths);

            match paths {
                Some((_cost, path)) => {
                    for (i, node_idx) in path.windows(2).enumerate() {
                        let src_node = g.node_weight(node_idx[0]).context("node_weight")?;
                        let dest_node = g.node_weight(node_idx[1]).context("node_weight")?;

                        let edge = g.find_edge(node_idx[0], node_idx[1]).context("find_edge")?;
                        let tx_id = g.edge_weight(edge).context("edge_weight")?;

                        let tx = sql_store.get_tx_data(tx_id)?.ok_or_err()?;
                        let tx_type = TxType::from_display(tx.tx_type.as_str());

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
        }
    }
    Ok(())
}
