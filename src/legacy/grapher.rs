use crate::args::Args;
use crate::db::{encode_height, rocks_open_db, RocksBlockStore};
use crate::dfiutils::extract_all_dfi_addresses;
use crate::lang::{Error, Result};
use crate::models::TxType;
use petgraph::graph::NodeIndex;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::info;

// Reduce graph with:
//
// for x in `seq 0 100000 2000000`; do
//  gvpr -c "N[$.degree==0]{delete(root, $)}" graph-${x}.dot > graph-${x}.cleaned.dot; # clean up nodes with no edges
//  cat graph-${x}.cleaned.dot | sed 's/ |.*\"\];/\"\];/' > graph-${x}.minimized.dot;  # remove edge labels
//  sfdp -x -Goverlap=scale -Tpdf graph-${x}.minimized.dot > ${x}.pdf; # render to pdf
// done
//

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxEdge {
    pub tx_type: TxType,
    pub tx_hash: String,
}

impl std::fmt::Display for TxEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "t:{} | {}",
            self.tx_type,
            self.tx_hash,
            // if self.tagged { "TAG" } else { "" }
        ))
    }
}

pub fn graph_it(args: Args) -> Result<()> {
    let logs_dir = args.graph_logs_path;
    std::fs::create_dir_all(&logs_dir)?;

    let db = rocks_open_db(None)?;
    let block_store = RocksBlockStore::new(&db)?;

    let start_key = "b/h/".to_owned() + &encode_height(0);
    let end_key = "b/h/".to_owned() + &encode_height(2_000_000);
    let iter = db.iterator(rust_rocksdb::IteratorMode::From(
        start_key.as_bytes(),
        rust_rocksdb::Direction::Forward,
    ));

    let mut g = petgraph::Graph::new();
    let mut node_map = HashMap::<String, NodeIndex>::default();

    let unknown_node = g.add_node("x".to_string());
    let coin_base_node = g.add_node("coinbase".to_string());

    node_map.insert("x".to_string(), unknown_node);
    node_map.insert("coinbase".to_string(), coin_base_node);

    let mut tagged_addrs = HashSet::<String>::default();

    let quit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&quit))?;

    let mut iz = 0;

    for (i, item) in iter.enumerate() {
        let (k, v) = item?;
        let key = std::str::from_utf8(&k)?;
        if !key.starts_with("b/h/") || *key > *end_key {
            info!("key prefix exceeded: {}", &key);
            break;
        }
        let h = std::str::from_utf8(&v)?;
        // info!(i, key, h);
        let b = block_store.get_block_from_hash(h)?;
        let block = b.ok_or_else(|| Error::from("block not found"))?;

        for x in block.tx {
            // info!("{:?}", &x);
            let mut tagged_dvm = false;

            let tx_data = block_store
                .get_tx_addr_data_from_hash(&x.txid)?
                .ok_or_else(|| Error::from(format!("tx data: {}", &x.txid)))?;

            // Note that there's only entry per unique address. It's already aggregated.
            let tx_in = tx_data.tx_in;
            let tx_out = tx_data.tx_out;

            let mut tx_type = x.vm.as_ref().map(|x| TxType::from(x.txtype.as_ref()));

            let mut dvm_addrs = vec![];

            if tx_in.is_empty() {
                tx_type = Some(TxType::Coinbase);
            }

            // We set dvm_addrs as well mark address to tag.
            if !matches!(
                &tx_type,
                Some(TxType::Coinbase) | Some(TxType::Unknown) | Some(TxType::Utxo) | None
            ) {
                let t = tx_type.clone().unwrap();
                let dvm_data = x.vm.as_ref().map(|x| x.msg.to_string()).unwrap();
                dvm_addrs = extract_all_dfi_addresses(&dvm_data)
                    .into_iter()
                    .collect::<Vec<_>>();

                if t == TxType::ICXMakeOffer
                    || t == TxType::ICXCloseOffer
                    || t == TxType::ICXCloseOrder
                    || t == TxType::ICXCreateOrder
                    || t == TxType::ICXClaimDFCHTLC
                    || t == TxType::ICXSubmitDFCHTLC
                    || t == TxType::ICXSubmitEXTHTLC
                {
                    tagged_dvm = true;
                } else {
                    for x in dvm_addrs.iter() {
                        if tagged_addrs.contains(x) {
                            tagged_dvm = true;
                            break;
                        }
                    }
                }
            }

            // immutable map that's cheap to clone with CoW semantics / im-rc uses RBR tree.
            // Edit: immutable map no longer used. ds of txedge is now super dumbed down
            // to just the txtype and hash, so can use std map now but switch to im-rc
            // if it gets complex again.
            //
            // let mut node_map_tx = node_map.clone();
            let node_map_tx = &mut node_map;

            for tx_in in tx_in.iter() {
                // strict checks, but we can't do partial analysis of the graph
                // if we enable it.
                // let v = node_map_tx.get_mut(tx_in.0).expect("tx in made-up");

                // We use contains instead of entry _or_insert_with, since the alloc here is more expensive
                // for each key as it'll check many existing keys over new key creations.
                if node_map_tx.contains_key(tx_in.0) {
                    continue;
                }
                let _v = node_map_tx.insert(tx_in.0.to_owned(), g.add_node(tx_in.0.to_owned()));
            }

            for tx_out in tx_out.iter() {
                if node_map_tx.contains_key(tx_out.0) {
                    continue;
                }
                let _v = node_map_tx.insert(tx_out.0.to_owned(), g.add_node(tx_out.0.to_owned()));
            }

            for x in dvm_addrs.iter() {
                if node_map_tx.contains_key(x) {
                    continue;
                }
                let _v = node_map_tx.insert(x.to_owned(), g.add_node(x.to_owned()));
            }

            let mut change_list = HashSet::<(NodeIndex, NodeIndex, Option<TxType>)>::default();

            for tx_out in tx_out.iter().filter(|x| x.0 != "x") {
                let tx_out_node_meta = node_map_tx.get(tx_out.0).unwrap();
                let tagged_out = if tagged_dvm || tagged_addrs.contains(tx_out.0) {
                    tagged_addrs.insert(tx_out.0.to_string());
                    true
                } else {
                    false
                };

                if tx_in.is_empty() && (tx_out.1 == &0.) {
                    if tagged_out {
                        change_list.insert((coin_base_node, *tx_out_node_meta, tx_type.clone()));
                    }
                    continue;
                }

                for tx_in in tx_in.iter() {
                    let tagged_in = if tagged_dvm || tagged_addrs.contains(tx_in.0) {
                        tagged_addrs.insert(tx_in.0.to_string());
                        true
                    } else {
                        false
                    };
                    if !tagged_in {
                        continue;
                    }

                    let in_node_meta = node_map_tx.get(tx_in.0).unwrap();
                    change_list.insert((*in_node_meta, *tx_out_node_meta, tx_type.clone()));
                }
            }

            for tx_in in tx_in.iter() {
                let in_node_meta = node_map_tx.get(tx_in.0).unwrap();
                for x in dvm_addrs.iter() {
                    if tagged_dvm || tagged_addrs.contains(x) {
                        change_list.insert((
                            *in_node_meta,
                            *node_map_tx.get(x).unwrap(),
                            tx_type.clone(),
                        ));
                    }
                }
            }

            for (in_node, out_node, tx_type) in change_list.into_iter() {
                g.add_edge(
                    in_node,
                    out_node,
                    TxEdge {
                        tx_type: tx_type.unwrap_or(TxType::Unknown),
                        tx_hash: x.txid.to_owned(),
                    },
                );
            }
            // node_map = node_map_tx;
        }
        iz = i;

        if i % 10000 == 0 {
            info!(i);
            if i % 100000 == 0 {
                let d = petgraph::dot::Dot::new(&g);
                info!("writing dot graph..");
                std::fs::write(format!("{}/graph-{}.dot", logs_dir, iz), format!("{}", d))?;
            }
        }

        if quit.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }
    }

    let d = petgraph::dot::Dot::new(&g);
    info!("writing dot graph..");
    std::fs::write(format!("{}/graph-{}.dot", logs_dir, iz), format!("{}", d))?;
    Ok(())
}
