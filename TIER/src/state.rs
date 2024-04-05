use crate::msg::{
    ContractStatus,
    OraiswapContract,
    QueryResponse,
    SerializedUnbonds,
    SerializedWithdrawals,
    ValidatorWithWeight,
};
use cosmwasm_std::{ StdError, StdResult, Storage, Uint128 };
use cw_storage_plus::{ Deque, Item, Map };
use serde::{ Deserialize, Serialize };

pub const CONFIG_ITEM: Item<Config> = Item::new("config");
pub const WITHDRAWALS_LIST: Map<String, Vec<UserWithdrawal>> = Map::new("withdraw"); //Deque<UserWithdrawal> = Deque::new("withdraw");
pub const UNBOND_LIST: Deque<UserUnbond> = Deque::new("unbond_list");
pub const USER_INFOS: Map<String, UserInfo> = Map::new("user_info");
pub const USER_TOTAL_DELEGATED: Map<String, Uint128> = Map::new("user_total_delegate");

// pub fn withdrawals_list(address: &CanonicalAddr) -> Deque<'static, UserWithdrawal> {
//     WITHDRAWALS_LIST.push_back(address.as_slice())
// }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub admin: String,
    pub validators: Vec<ValidatorWithWeight>,
    pub status: u8,
    pub usd_deposits: Vec<u128>,
    pub oraiswap_contract: OraiswapContract,
    pub stable_denom: Vec<String>,
}

impl Config {
    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG_ITEM.load(storage)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG_ITEM.save(storage, self)
    }

    pub fn min_tier(&self) -> u8 {
        self.usd_deposits.len().checked_add(1).unwrap() as u8
    }

    pub fn max_tier(&self) -> u8 {
        1
    }

    pub fn deposit_by_tier(&self, tier: u8) -> u128 {
        let tier_index = tier.checked_sub(1).unwrap();
        self.usd_deposits[tier_index as usize]
    }

    pub fn tier_by_deposit(&self, usd_deposit: u128) -> u8 {
        self.usd_deposits
            .iter()
            .position(|d| *d <= usd_deposit)
            .unwrap_or(self.usd_deposits.len())
            .checked_add(1)
            .unwrap() as u8
    }

    pub fn assert_contract_active(&self) -> StdResult<()> {
        let active = ContractStatus::Active as u8;
        if self.status != active {
            return Err(StdError::generic_err("Contract is not active"));
        }

        Ok(())
    }

    pub fn to_answer(&self) -> StdResult<QueryResponse> {
        let admin = self.admin.clone(); //api.addr_humanize(&self.admin)?;
        let min_tier = self.usd_deposits.len().checked_add(1).unwrap() as u8;

        return Ok(QueryResponse::Config {
            admin,
            min_tier,
            validators: self.validators.clone(),
            oraiswap_contract: self.oraiswap_contract.clone(),
            status: self.status.into(),
            usd_deposits: self.usd_deposits
                .iter()
                .map(|d| Uint128::from(*d))
                .collect(),
            stable_denom: self.stable_denom.clone(),
        });
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserInfo {
    pub tier: u8,
    pub timestamp: u64,
    pub usd_deposit: u128,
    pub orai_deposit: u128,
    pub total_orai_deposit: u128,
}

impl UserInfo {
    pub fn to_answer(&self) -> QueryResponse {
        QueryResponse::UserInfo {
            tier: self.tier,
            timestamp: self.timestamp,
            usd_deposit: Uint128::from(self.usd_deposit),
            orai_deposit: Uint128::from(self.orai_deposit),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserWithdrawal {
    pub amount: u128,
    pub claim_time: u64,
    pub timestamp: u64,
}

impl UserWithdrawal {
    pub fn to_serialized(&self) -> SerializedWithdrawals {
        SerializedWithdrawals {
            amount: Uint128::from(self.amount),
            claim_time: self.claim_time,
            timestamp: self.timestamp,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserUnbond {
    pub address: String,
    pub amount: u128,
    pub timestamp: u64,
}

impl UserUnbond {
    pub fn to_serialized(&self) -> SerializedUnbonds {
        SerializedUnbonds {
            amount: Uint128::from(self.amount),
            address: self.address.clone(),
            timestamp: self.timestamp,
        }
    }
}
