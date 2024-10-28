#![allow(dead_code)]

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub hash: String,
    pub height: u32,
    pub confirmations: i64,
    pub strippedsize: u64,
    pub size: u64,
    pub weight: u64,
    pub minter: MinterInfo,
    pub version: i32,
    pub version_hex: String,
    pub merkleroot: String,
    pub time: i64,
    pub mediantime: i64,
    pub bits: String,
    pub difficulty: f64,
    pub chainwork: String,
    pub tx: Vec<Transaction>,
    pub n_tx: u64,
    pub previousblockhash: Option<String>,
    pub nextblockhash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MinterInfo {
    pub id: String,
    pub operator: Option<String>,
    pub owner: Option<String>,
    pub reward_address: Option<String>,
    pub total_minted: u64,
    pub stake_modifier: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub txid: String,
    pub hash: String,
    pub version: u32,
    pub size: u64,
    pub vsize: u64,
    pub weight: u64,
    pub locktime: u64,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    pub hex: String,
    pub vm: Option<VMInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VMInfo {
    pub vmtype: String,
    pub txtype: String,
    pub msg: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSig {
    asm: String,
    pub hex: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Vin {
    Coinbase(VinCoinbase),
    Standard(VinStandard),
}

impl Vin {
    pub fn _assume_coinbase(&self) -> Option<VinCoinbase> {
        match self {
            Vin::Coinbase(x) => Some(x.clone()),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn assume_standard(&self) -> Option<VinStandard> {
        match self {
            Vin::Standard(x) => Some(x.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VinCoinbase {
    pub coinbase: String,
    pub sequence: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VinStandard {
    pub txid: String,
    pub vout: u64,
    pub script_sig: ScriptSig,
    pub txinwitness: Option<Vec<String>>,
    pub sequence: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Vout {
    pub value: f64,
    pub n: u64,
    pub script_pub_key: ScriptPubKey,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPubKey {
    pub asm: String,
    pub hex: String,
    pub r#type: String,
    pub req_sigs: Option<u64>,
    pub addresses: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IcxLogData {
    pub order_tx: String,
    pub offer_tx: String,
    pub dfchtlc_tx: String,
    pub claim_tx: String,
    pub address: String,
    pub amount: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IcxTxSet<'a> {
    pub order_tx: Cow<'a, str>,
    pub offer_tx: Cow<'a, str>,
    pub dfchtlc_tx: Cow<'a, str>,
    pub claim_tx: Cow<'a, str>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TxType {
    Unknown,
    Coinbase,
    Utxo,
    AutoAuth,
    UtxosToAccount,
    AccountToUtxos,
    AccountToAccount,
    AnyAccountsToAccounts,
    CreateMasternode,
    ResignMasternode,
    PoolSwap,
    CompositeSwap,
    AddPoolLiquidity,
    RemovePoolLiquidity,
    WithdrawFromVault,
    SetOracleData,
    DepositToVault,
    PaybackLoan,
    TakeLoan,
    Vault,
    ICXCreateOrder,
    ICXMakeOffer,
    ICXSubmitDFCHTLC,
    ICXSubmitEXTHTLC,
    ICXClaimDFCHTLC,
    ICXCloseOrder,
    ICXCloseOffer,
    Other(String),
}

impl From<&str> for TxType {
    fn from(value: &str) -> Self {
        use TxType::*;
        match value {
            "_" => Unknown,
            "cb" => Coinbase,
            "utxo" => Utxo,
            "CreateMasternode" => CreateMasternode,
            "ResignMasternode" => ResignMasternode,
            "PoolSwap" => PoolSwap,
            "CompositeSwap" => CompositeSwap,
            "AddPoolLiquidity" => AddPoolLiquidity,
            "RemovePoolLiquidity" => RemovePoolLiquidity,
            "UtxosToAccount" => UtxosToAccount,
            "AccountToUtxos" => AccountToUtxos,
            "AccountToAccount" => AccountToAccount,
            "WithdrawFromVault" => WithdrawFromVault,
            "SetOracleData" => SetOracleData,
            "DepositToVault" => DepositToVault,
            "PaybackLoan" => PaybackLoan,
            "TakeLoan" => TakeLoan,
            "AutoAuth" => AutoAuth,
            "Vault" => Vault,
            "AnyAccountsToAccounts" => AnyAccountsToAccounts,
            "ICXCreateOrder" => ICXCreateOrder,
            "ICXMakeOffer" => ICXMakeOffer,
            "ICXSubmitDFCHTLC" => ICXSubmitDFCHTLC,
            "ICXSubmitEXTHTLC" => ICXSubmitEXTHTLC,
            "ICXClaimDFCHTLC" => ICXClaimDFCHTLC,
            "ICXCloseOrder" => ICXCloseOrder,
            "ICXCloseOffer" => ICXCloseOffer,
            other => Other(other.to_owned()),
        }
    }
}

impl std::fmt::Display for TxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TxType::*;
        let t = match self {
            Unknown => "_",
            Coinbase => "cb",
            Utxo => "u",
            AutoAuth => "au",
            UtxosToAccount => "+a",
            AccountToUtxos => "-a",
            AccountToAccount => "aa",
            AnyAccountsToAccounts => "ax",
            CreateMasternode => "+m",
            ResignMasternode => "-m",
            PoolSwap => "ps",
            CompositeSwap => "cs",
            AddPoolLiquidity => "+p",
            RemovePoolLiquidity => "-p",
            WithdrawFromVault => "v-",
            DepositToVault => "v+",
            PaybackLoan => "l-",
            TakeLoan => "l+",
            Vault => "vn",
            SetOracleData => "+o",
            ICXCreateOrder => "icx-start",
            ICXMakeOffer => "icx-of",
            ICXSubmitDFCHTLC => "icx-sdfc",
            ICXSubmitEXTHTLC => "icx-sbtc",
            ICXClaimDFCHTLC => "icx-claim",
            ICXCloseOrder => "icx-endor",
            ICXCloseOffer => "icx-endof",
            Other(m) => m,
        };
        f.write_str(t)
    }
}

type TokenAmount = String;

// vm":{"vmtype":"dvm","txtype":"UtxosToAccount","msg":{"8RbpgySS2qkXQG2UosQCqADtS7zRAr8bx5":"60000.00000000@0"}}}
pub type UtxosToAccountMsg = HashMap<String, TokenAmount>;

// "vm":{"vmtype":"dvm","txtype":"AccountToAccount","msg":{"from":"dK13qHWrbSdtFkxnfg3UVEvNrsxa9i45pd","to":{"dc432ofNoMBg3Y6eubzx5dS1iRLMKXsBWE":"2.00000000@128"}}}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountToAccountMsg {
    pub from: String,
    pub to: HashMap<String, TokenAmount>,
}

// "vm":{"vmtype":"dvm","txtype":"AnyAccountsToAccounts","msg":{"from":{"dPhcSbZFcqeiaKxpVc9yWGTGchgvfXvFA8":"1.00000000@0"},"to":{"8VW5syUUa726cPYUjidE7SyyGjEZrVi4JU":"1.00000000@0"}}}}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnyAccountsToAccountsMsg {
    pub from: HashMap<String, TokenAmount>,
    pub to: HashMap<String, TokenAmount>,
}

// "vm":{"vmtype":"dvm","txtype":"AccountToUtxos","msg":{"from":"8HzyWaC9bJKCouveUed2jm8w4MJzrt3c2Q","to":{"dFZRkToyEgnWy8GSXHmJPM1KXY67XKgSQx":"6338.00000000@0"}}}}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountToUtxosMsg {
    pub from: String,
    pub to: HashMap<String, TokenAmount>,
}

// "vm":{"vmtype":"dvm","txtype":"PoolSwap","msg":{"fromAddress":"8J6KKxHQAWDJDR1PQfC46ocgmxTvtLLc6R","fromAmount":9.0,"fromToken":"0","maxPrice":0.00002531,"maxPriceHighPrecision":"0.00002531","toAddress":"8eG9Pe1wQnWZuXD5NRr3QaxDex9RJ99fd5","toToken":"2"}}}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct PoolSwapMsg {
    pub from_address: String,
    pub to_address: String,
    pub from_amount: f64,
    pub from_token: String,
    pub to_token: String,
}
