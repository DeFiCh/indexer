#![allow(dead_code)]

use crate::db::BlockStore;
use crate::lang::Error;
use crate::models::{TStr, Transaction, Vin, VinStandard, Vout};
use crate::Result;
use core::str;
use std::collections::{HashMap, HashSet};
use std::process::{Command, Output};
use std::rc::Rc;
use tracing::warn;

#[derive(Debug)]
pub struct CliDriver {
    pub cli_path: String,
}

pub struct OutputExt {
    output: Output,
}

impl OutputExt {
    pub fn str(&self) -> Result<std::rc::Rc<str>> {
        Ok(std::rc::Rc::from(std::str::from_utf8(&self.output.stdout)?))
    }

    pub fn json<'a, T>(&'a self) -> Result<T>
    where
        T: serde::Deserialize<'a>,
    {
        Ok(serde_json::from_slice(&self.output.stdout)?)
    }
}

impl CliDriver {
    pub fn new() -> CliDriver {
        CliDriver {
            cli_path: "defi-cli".to_owned(),
        }
    }

    pub fn with_cli_path(cli_path: String) -> CliDriver {
        CliDriver { cli_path }
    }

    pub fn run<I, S>(&mut self, args: I) -> Result<OutputExt>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let res = Command::new(&self.cli_path).args(args).output()?;
        if !res.status.success() {
            let err = String::from_utf8_lossy(&res.stderr);
            return Err(err.into());
        }
        Ok(OutputExt { output: res })
    }

    pub fn get_block_count(&mut self) -> Result<i64> {
        let out = self.run(["getblockcount"])?;
        let res = out.str()?;
        Ok(res.trim().parse::<i64>()?)
    }

    pub fn get_block_hash(&mut self, height: i64) -> Result<Rc<str>> {
        let out = self.run(["getblockhash", &height.to_string()])?;
        Ok(Rc::from(out.str()?.trim()))
    }

    pub fn get_block(&mut self, hash: &str, verbosity: Option<i32>) -> Result<OutputExt> {
        let mut args = Vec::from(["getblock", hash]);
        let v_str;
        if let Some(v) = verbosity {
            v_str = Some(v.to_string());
            args.push(v_str.as_ref().unwrap());
        }
        self.run(args)
    }
}

pub fn extract_all_dfi_addresses(json_haystack: &str) -> HashSet<TStr> {
    use std::sync::LazyLock;
    static DFI_ADDRESS_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        let r1 = r#""(d|7|8)[1-9A-HJ-NP-Za-km-z]{25,34}""#; // legacy
        let r2 = r#""df1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{38,87}""#; // bech32
        let s = [r1, r2].join("|");
        regex::Regex::new(&s).unwrap()
    });

    DFI_ADDRESS_RE
        .captures_iter(json_haystack)
        .map(|x| TStr::from(x[0].trim_matches('\"'))) // remove quotes
        .collect::<HashSet<_>>() // unique
}

#[test]
fn test_extract_dfi_addresses() {
    let json_haystack = r#"
            {
                        "txid": "8842e454dcc8021cf2a74200a2154c795fc712fa4f6e035c7eaa5be744601b0a"
                        "fromAddress": "8J6KKxHQAWDJDR1PQfC46ocgmxTvtLLc6R",
                        "randomNonAddress": "8842e829d6f1969eb9c22f29b5d8ccc44725b5",
                        "dfchtlcTx": "0e7c00dec3377b3099d25ca2b8d0a12829d6f1969eb9c22f29b5d8ccc44725b5",
                        "ttx": "525202f6ff4d7480e180694bccd201902c97f2df438e8ad95f4de22b48667527",
                        "seed": "b11d186beb4284afa5261d7c662e998aeafcedaed114f0b18045b7533d9edad4",
                        "test": "df1qqvaqshw0hrjzakxms27xrk6npfef4sx6cqaejv",
                        "test2": "dazewCkFnaw4o67RQrS5FATMKy9mAcohNA",
                        "test3": "dZcuogFeLxy5NLFZnShYiX2sp9M6vv6UKj",
                        "test4": "8aQxUdEUxiffqxy4eqqepYMdPUw3sGQiA2",
                        "fromAmount": 9.0,
                        "fromToken": "0",
                        "maxPrice": 2.531e-05,
                        "maxPriceHighPrecision": "0.00002531",
                        "toAddress": "8eG9Pe1wQnWZuXD5NRr3QaxDex9RJ99fd5",
                        "toToken": "2"
            }
        "#;

    let mut expected = vec![
        "8J6KKxHQAWDJDR1PQfC46ocgmxTvtLLc6R",
        "df1qqvaqshw0hrjzakxms27xrk6npfef4sx6cqaejv",
        "dazewCkFnaw4o67RQrS5FATMKy9mAcohNA",
        "dZcuogFeLxy5NLFZnShYiX2sp9M6vv6UKj",
        "8aQxUdEUxiffqxy4eqqepYMdPUw3sGQiA2",
        "8eG9Pe1wQnWZuXD5NRr3QaxDex9RJ99fd5",
    ];

    expected.sort();

    let mut addresses = extract_all_dfi_addresses(json_haystack)
        .into_iter()
        .collect::<Vec<_>>();

    addresses.sort();

    for x in addresses.iter().zip(expected) {
        assert_eq!(x.0.as_ref(), x.1);
    }
}

pub fn token_id_to_symbol_maybe(token_id: &str) -> &str {
    match token_id {
        "0" => "dfi",
        "1" => "eth",
        "2" => "btc",
        "3" => "usdt",
        "7" => "doge",
        "9" => "ltc",
        "11" => "bch",
        "13" => "usdc",
        "15" => "dusd",
        _ => token_id,
    }
}

pub fn get_txin_addr_val_list(
    tx_ins: &[Vin],
    block_store: &impl BlockStore,
) -> Result<Vec<(TStr, f64)>> {
    let map_fn = |x: VinStandard| {
        let tx_id = x.txid;
        let tx = block_store.get_tx_from_hash(&tx_id);
        let tx = tx?.ok_or_else(|| Error::from(format!("tx hash not found: {}", &tx_id)))?;
        let utxo = tx
            .vout
            .iter()
            .find(|v| v.n == x.vout)
            .ok_or_else(|| Error::from(format!("tx vout not found: {}", &tx_id)))?;
        let val = utxo.value;
        if let Some(addrs) = &utxo.script_pub_key.addresses {
            if addrs.len() == 1 {
                return Ok((addrs[0].clone(), val));
            } else {
                warn!("multiple addresses found: {}", tx_id);
            }
            // Multi-sig, we just join it with a +
            let s = addrs.join("+");
            Ok((TStr::from(s), val))
        } else {
            Err(Error::from(format!("input with no addr found: {}", tx_id)))
        }
    };

    tx_ins
        .iter()
        .filter_map(Vin::assume_standard)
        .map(map_fn)
        .collect()
}

pub fn get_txout_addr_val_list(tx: &Transaction, tx_outs: &[Vout]) -> Vec<(TStr, f64)> {
    tx_outs
        .iter()
        .map(|utxo| {
            let val = utxo.value;
            let addr = if let Some(addrs) = &utxo.script_pub_key.addresses {
                if addrs.len() > 1 {
                    warn!("multiple addresses found: {}", tx.txid);
                }
                // Multi-sig, we just join it with a +
                TStr::from(addrs.join("+"))
            } else {
                // most dvm OP_RETURN txs without address will be these
                TStr::from("x")
            };
            (addr, val)
        })
        .collect::<Vec<_>>()
}

pub fn fold_addr_val_map(addr_val_list: &[(TStr, f64)]) -> HashMap<TStr, f64> {
    addr_val_list
        .iter()
        .fold(HashMap::with_capacity(addr_val_list.len()), |mut m, v| {
            m.entry(v.0.clone())
                .and_modify(|x| *x += v.1)
                .or_insert(v.1);
            m
        })
}
