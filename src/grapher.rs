use crate::db::SqliteBlockStore;
use crate::lang::Result;
use anyhow::Context;
use clap::Parser;
use std::collections::{HashMap, HashSet};
use tracing::{debug, error, info};

#[derive(Parser, Debug)]
pub struct GrapherArgs {
    #[arg(long, default_value = "data/index.sqlite")]
    pub sqlite_path: String,
    #[arg(long, default_value = "data/graph.bin")]
    pub graph_data_path: String,
    #[arg(long, default_value = "data/graph.meta.bin")]
    pub graph_meta_path: String,
    #[arg(short = 's', long, default_value_t = 0)]
    pub start_height: i64,
    #[arg(short = 'e', long, default_value_t = 2_000_000)]
    pub end_height: i64,
}

pub fn run(args: &GrapherArgs) -> Result<()> {
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
    let mut txiter = 0;

    let mut g = petgraph::Graph::new();
    let mut node_index_map = std::collections::HashMap::<String, _>::new();

    let r = sql_store.iter_txs(None, |tx| {
        if quit.load(std::sync::atomic::Ordering::Relaxed) {
            info!("int: early exit");
            return Err("interrupted".into());
        }

        txiter += 1;
        let tx = tx?;

        fn combine_addrs_with_multi_sig<'a, T1, T2>(
            addresses: T1,
            dvm_addresses: T2,
        ) -> HashSet<String>
        where
            T1: Iterator<Item = &'a str>,
            T2: Iterator<Item = &'a str>,
        {
            let mut set = HashSet::new();
            for addr in addresses {
                if addr.contains('+') {
                    // Multi-sig, we include each of them for the graph
                    for part in addr.split('+') {
                        set.insert(part.to_owned());
                    }
                } else {
                    set.insert(addr.to_owned());
                }
            }
            for dvm_addr in dvm_addresses {
                set.insert(dvm_addr.to_owned());
            }
            set
        }

        let tx_ins = combine_addrs_with_multi_sig(
            tx.tx_in.keys().map(|s| s.as_str()),
            tx.dvm_in.iter().map(|s| s.as_str()),
        );
        let tx_outs = combine_addrs_with_multi_sig(
            tx.tx_out.keys().map(|s| s.as_str()),
            tx.dvm_out.iter().map(|s| s.as_str()),
        );

        // Create nodes for any new addresses
        for addr in tx_ins.iter().chain(tx_outs.iter()) {
            if !node_index_map.contains_key(addr) {
                let node_idx = g.add_node(addr.clone());
                node_index_map.insert(addr.clone(), node_idx);
            }
        }

        // Add edges between inputs and outputs
        for to_addr in &tx_outs {
            for from_addr in &tx_ins {
                let from_idx = node_index_map[from_addr];
                let to_idx = node_index_map[to_addr];
                g.add_edge(from_idx, to_idx, tx.txid.clone());
            }
        }

        if txiter % 100000 == 0 {
            info!(
                "txiter: {}, nodes: {}, edges: {}",
                txiter,
                g.node_count(),
                g.edge_count()
            );
        }

        if user_sig.load(std::sync::atomic::Ordering::Relaxed) {
            info!("sig received: dumping memory");
            user_sig.store(false, std::sync::atomic::Ordering::Release);
            dump_graph_data(
                txiter,
                &g,
                &node_index_map,
                &args.graph_meta_path,
                &args.graph_data_path,
            )?;
        }

        Ok(())
    });

    if let Err(e) = r {
        if e.to_string() == "interrupted" {
            info!("{:?}", e);
        } else {
            error!("{:?}", e);
        }
    } else {
        dump_graph_data(
            txiter,
            &g,
            &node_index_map,
            &args.graph_meta_path,
            &args.graph_data_path,
        )?;
    }

    info!("summary: scanned txs: {}", txiter);
    Ok(())
}

fn dump_graph_data(
    txiter: i32,
    g: &petgraph::Graph<String, String>,
    node_index_map: &std::collections::HashMap<String, petgraph::graph::NodeIndex>,
    meta_path: &str,
    data_path: &str,
) -> crate::lang::Result<()> {
    info!(
        "txiter: {}, nodes: {}, edges: {}",
        txiter,
        g.node_count(),
        g.edge_count()
    );
    info!("writing graph metadata to {}..", meta_path);
    let f = std::fs::File::create(meta_path)?;
    let f = std::io::BufWriter::with_capacity(1 << 26, f); // 64mb
    bincode::serialize_into(f, &node_index_map).context("meta bincode ser err")?;
    // serde_json::to_writer(f, &node_index_map)?;
    info!("writing graph data to {}..", data_path);
    let f = std::fs::File::create(data_path)?;
    let f = std::io::BufWriter::with_capacity(1 << 26, f); // 64mb
    bincode::serialize_into(f, &g).context("g bincode ser err")?;
    // serde_json::to_writer(f, &g)?;
    info!("done");
    Ok(())
}
