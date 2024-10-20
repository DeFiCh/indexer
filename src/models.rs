#![allow(dead_code)]

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct IcxTxSet<'a> {
    pub order_tx: &'a str,
    pub offer_tx: &'a str,
    pub dfchtlc_tx: &'a str,
    pub claim_tx: &'a str,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TxType {
    Unknown,
    Coinbase,
    Utxo,
    CreateMasternode,
    ResignMasternode,
    PoolSwap,
    CompositeSwap,
    AddPoolLiquidity,
    RemovePoolLiquidity,
    UtxosToAccount,
    AccountToUtxos,
    AccountToAccount,
    WithdrawFromVault,
    SetOracleData,
    DepositToVault,
    PaybackLoan,
    TakeLoan,
    AutoAuth,
    Vault,
    AnyAccountsToAccounts,
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
            CreateMasternode => "+m",
            ResignMasternode => "-m",
            PoolSwap => "ps",
            CompositeSwap => "cs",
            AddPoolLiquidity => "+p",
            RemovePoolLiquidity => "-p",
            UtxosToAccount => "+a",
            AccountToUtxos => "-a",
            AccountToAccount => "aa",
            SetOracleData => "+o",
            AnyAccountsToAccounts => "ax",
            AutoAuth => "au",
            WithdrawFromVault => "v-",
            DepositToVault => "v+",
            PaybackLoan => "l-",
            TakeLoan => "l+",
            Vault => "vn",
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
