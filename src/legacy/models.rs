#![allow(dead_code)]

use serde::{Deserialize, Serialize};

pub type TStr = String;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub hash: TStr,
    pub height: u32,
    pub confirmations: i64,
    pub strippedsize: u64,
    pub size: u64,
    pub weight: u64,
    pub minter: MinterInfo,
    pub version: i32,
    pub version_hex: TStr,
    pub merkleroot: TStr,
    pub time: i64,
    pub mediantime: i64,
    pub bits: TStr,
    pub difficulty: f64,
    pub chainwork: TStr,
    pub tx: Vec<Transaction>,
    pub n_tx: u64,
    pub previousblockhash: Option<TStr>,
    pub nextblockhash: Option<TStr>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MinterInfo {
    pub id: TStr,
    pub operator: Option<TStr>,
    pub owner: Option<TStr>,
    pub reward_address: Option<TStr>,
    pub total_minted: u64,
    pub stake_modifier: TStr,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub txid: TStr,
    pub hash: TStr,
    pub version: u32,
    pub size: u64,
    pub vsize: u64,
    pub weight: u64,
    pub locktime: u64,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    pub hex: TStr,
    pub vm: Option<VMInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VMInfo {
    pub vmtype: TStr,
    pub txtype: TStr,
    pub msg: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSig {
    asm: TStr,
    pub hex: Option<TStr>,
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
    pub coinbase: TStr,
    pub sequence: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VinStandard {
    pub txid: TStr,
    pub vout: u64,
    pub script_sig: ScriptSig,
    pub txinwitness: Option<Vec<TStr>>,
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
    pub asm: TStr,
    pub hex: TStr,
    pub r#type: TStr,
    pub req_sigs: Option<u64>,
    pub addresses: Option<Vec<TStr>>,
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

impl TxType {
    pub fn from_display(s: &str) -> Self {
        match s {
            "_" => TxType::Unknown,
            "cb" => TxType::Coinbase,
            "u" => TxType::Utxo,
            "au" => TxType::AutoAuth,
            "+a" => TxType::UtxosToAccount,
            "-a" => TxType::AccountToUtxos,
            "aa" => TxType::AccountToAccount,
            "ax" => TxType::AnyAccountsToAccounts,
            "+m" => TxType::CreateMasternode,
            "-m" => TxType::ResignMasternode,
            "ps" => TxType::PoolSwap,
            "cs" => TxType::CompositeSwap,
            "+p" => TxType::AddPoolLiquidity,
            "-p" => TxType::RemovePoolLiquidity,
            "v-" => TxType::WithdrawFromVault,
            "v+" => TxType::DepositToVault,
            "l-" => TxType::PaybackLoan,
            "l+" => TxType::TakeLoan,
            "vn" => TxType::Vault,
            "+o" => TxType::SetOracleData,
            "icx-start" => TxType::ICXCreateOrder,
            "icx-of" => TxType::ICXMakeOffer,
            "icx-sdfc" => TxType::ICXSubmitDFCHTLC,
            "icx-sbtc" => TxType::ICXSubmitEXTHTLC,
            "icx-claim" => TxType::ICXClaimDFCHTLC,
            "icx-endor" => TxType::ICXCloseOrder,
            "icx-endof" => TxType::ICXCloseOffer,
            other => TxType::Other(other.to_owned()),
        }
    }
}
