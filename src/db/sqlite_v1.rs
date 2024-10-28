#![allow(dead_code)]

use crate::lang::Result;
use rusqlite::{CachedStatement, Connection};

use crate::db::sqlite_init_pragma_v1;

pub fn sqlite_init_db_v1(path: Option<&str>) -> Result<Connection> {
    let path = path.unwrap_or("data/index.sqlite");
    let conn = rusqlite::Connection::open(path)?;
    sqlite_init_pragma_v1(&conn)?;
    sqlite_init_tables_v1(&conn)?;
    Ok(conn)
}

fn sqlite_init_tables_v1(conn: &Connection) -> Result<()> {
    // height is coalesced into rowid, so height is stored in the btree
    // and rest is stored on the leaf data page.
    // Note: We add the unique index directly in table to ensure lookups
    // can happen while indexing.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blocks (
            height INTEGER PRIMARY KEY,
            hash TEXT UNIQUE NOT NULL,
            data TEXT NOT NULL
        )",
        [],
    )?;

    // Note that using text as primary is similar to just an additional
    // index as sqlite will add implicit rowid as the btree* key.
    // We want this as rowid (int), is significantly cheaper to add other
    // indexes on top.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS txs (
            txid TEXT PRIMARY KEY,
            height INTEGER NOT NULL,
            tx_type TEXT NOT NULL,
            tx_in TEXT NOT NULL,
            tx_out TEXT NOT NULL,
            dvm_addrs TEXT NOT NULL,
            data TEXT NOT NULL,
            icx_data TEXT NOT NULL,
            icx_addr TEXT NOT NULL,
            icx_btc_exp_amt TEXT NOT NULL,
            swap_from TEXT NOT NULL,
            swap_to TEXT NOT NULL,
            swap_amt TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tx_graph (
            rowid INTEGER PRIMARY KEY,
            tx_in_addr TEXT NOT NULL,
            txid TEXT NOT NULL,
            tx_out_addr TEXT NOT NULL,
            c_flags TEXT NOT NULL
        )",
        [],
    )?;

    Ok(())
}

pub fn sqlite_create_index_factory_v1(
    conn: &rusqlite::Connection,
) -> impl Iterator<Item = (&str, impl Fn() -> rusqlite::Result<()> + '_)> {
    let indexes = vec![
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_height ON txs (height)",
            "idx_txs_height",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_tx_type ON txs (tx_type)",
            "idx_txs_tx_type",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_icx_addr ON txs (icx_addr)",
            "idx_txs_icx_addr",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_swap_from ON txs (swap_from)",
            "idx_txs_swap_from",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_txs_swap_to ON txs (swap_to)",
            "idx_txs_swap_to",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_tx_graph_tx_in_addr ON tx_graph (tx_in_addr)",
            "idx_tx_graph_tx_in_addr",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_tx_graph_tx_out_addr ON tx_graph (tx_out_addr)",
            "idx_tx_graph_tx_out_addr",
        ),
        (
            "CREATE INDEX IF NOT EXISTS idx_tx_graph_txid ON tx_graph (txid)",
            "idx_tx_graph_txid",
        ),
    ];

    let mut itr = indexes.into_iter();

    std::iter::from_fn(move || {
        if let Some((query, name)) = itr.next() {
            let closure = Box::new(|| conn.execute(query, []).map(|_| ()));
            return Some((name, closure));
        }
        None
    })
}

pub fn sqlite_get_stmts_v1(conn: &rusqlite::Connection) -> Result<[CachedStatement<'_>; 3]> {
    let insert_block_stmt = conn.prepare_cached(
        "
        insert or replace into blocks (height, hash, data)
        values (?1, ?2, ?3)
    ",
    )?;

    let insert_tx_stmt = conn.prepare_cached(
        "
        insert or replace into txs (
            txid, height, tx_type, tx_in, tx_out, dvm_addrs, data, icx_data, icx_addr, icx_btc_exp_amt, swap_from, swap_to, swap_amt
        )
        values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
    ",
    )?;

    let insert_tx_graph_stmt = conn.prepare_cached(
        "
        insert or replace into tx_graph (tx_in_addr, txid, tx_out_addr, c_flags)
        values (?1, ?2, ?3, ?4)
    ",
    )?;

    Ok([insert_block_stmt, insert_tx_stmt, insert_tx_graph_stmt])
}
