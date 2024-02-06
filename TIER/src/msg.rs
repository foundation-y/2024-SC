use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ContractStatus {
    Active,
    Stopped,
}

impl From<u8> for ContractStatus {
    fn from(status: u8) -> Self {
        if status == ContractStatus::Active as u8 {
            ContractStatus::Active
        } else if status == ContractStatus::Stopped as u8 {
            ContractStatus::Stopped
        } else {
            panic!("Wrong status");
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub validators: Vec<ValidatorWithWeight>,
    pub deposits: Vec<Uint128>,
    pub oraiswap_contract: OraiswapContract,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ChangeAdmin {
        admin: String,
    },
    ChangeStatus {
        status: ContractStatus,
    },
    Deposit {},
    Withdraw {},
    Claim {
        recipient: Option<String>,
        start: Option<u32>,
        limit: Option<u32>,
    },
    WithdrawRewards {
        recipient: Option<String>,
    },
    Redelegate {
        validator_address: String,
        recipient: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteResponse {
    ChangeAdmin {
        status: ResponseStatus,
    },
    ChangeStatus {
        status: ResponseStatus,
    },
    Deposit {
        usd_deposit: Uint128,
        orai_deposit: Uint128,
        tier: u8,
        status: ResponseStatus,
    },
    Withdraw {
        status: ResponseStatus,
    },
    Claim {
        amount: Uint128,
        status: ResponseStatus,
    },
    WithdrawRewards {
        amount: Uint128,
        status: ResponseStatus,
    },
    Redelegate {
        amount: Uint128,
        status: ResponseStatus,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    UserInfo {
        address: String,
    },
    Withdrawals {
        address: String,
        start: Option<u32>,
        limit: Option<u32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SerializedWithdrawals {
    pub amount: Uint128,
    pub claim_time: u64,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryResponse {
    Config {
        admin: String,
        validators: Vec<ValidatorWithWeight>,
        status: ContractStatus,
        usd_deposits: Vec<Uint128>,
        min_tier: u8,
        oraiswap_contract: OraiswapContract,
    },
    UserInfo {
        tier: u8,
        timestamp: u64,
        usd_deposit: Uint128,
        orai_deposit: Uint128,
    },
    Withdrawals {
        amount: u32,
        withdrawals: Vec<SerializedWithdrawals>,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ValidatorWithWeight {
    pub address: String,
    pub weight: u128,
}

impl Clone for ValidatorWithWeight {
    fn clone(&self) -> ValidatorWithWeight {
        ValidatorWithWeight {
            address: self.address.clone(),
            weight: self.weight.clone(), // Handle other fields accordingly.
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct OraiswapContract {
    pub orai_swap_router_contract: String,
    pub usdt_contract: String,
}

impl Clone for OraiswapContract {
    fn clone(&self) -> OraiswapContract {
        OraiswapContract {
            orai_swap_router_contract: self.orai_swap_router_contract.clone(),
            usdt_contract: self.usdt_contract.clone(),
        }
    }
}
