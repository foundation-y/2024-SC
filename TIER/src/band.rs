use crate::state::Config;
use cosmwasm_std::DepsMut;
use cosmwasm_std::StdResult;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub struct OraiPriceOracle {
    exchange_rate: u128,
}

impl OraiPriceOracle {
    pub const ZERO_12: u128 = 1_000_000_000_000;
    pub const ZERO_6: u128 = 1_000_000;

    pub fn new(deps: &DepsMut) -> StdResult<Self> {
        let config = Config::load(deps.storage)?;
        let orai_swap_router_contract = config.oraiswap_contract.orai_swap_router_contract;
        let native_token = NativeToken::new("orai".to_string());
        let offer_asset_info = OfferAssetInfo::new(native_token);
        let usdt_contract_address = config.oraiswap_contract.usdt_contract;
        let msg = SwapContractMessage {
            simulate_swap_operations: SwapContractMessageContent {
                offer_amount: 1000000,
                operations: vec![Operation {
                    orai_swap: OraiSwap {
                        offer_asset_info: offer_asset_info,
                        ask_asset_info: AskAssetInfo {
                            token: UsdtContractAddr {
                                contract_addr: usdt_contract_address,
                            },
                        },
                    },
                }],
            },
        };
        let response: ExchangeRateResponse = deps
            .querier
            .query_wasm_smart(orai_swap_router_contract, &msg)?;
        let exchange_rate = response.amount;

        Ok(OraiPriceOracle { exchange_rate })
    }

    pub fn usd_amount(&self, orai: u128) -> u128 {
        orai.checked_mul(self.exchange_rate)
            .and_then(|v| v.checked_div(OraiPriceOracle::ZERO_12))
            .unwrap()
    }

    pub fn orai_amount(&self, usd: u128) -> u128 {
        usd.checked_mul(OraiPriceOracle::ZERO_12)
            .and_then(|v: u128| v.checked_div(self.exchange_rate))
            .and_then(|v: u128| v.checked_add(1))
            .unwrap()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
// Define the struct for the innermost part "native_token"
pub struct NativeToken {
    denom: String,
}

impl NativeToken {
    pub fn new(native_token_denom: String) -> Self {
        NativeToken {
            denom: native_token_denom,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
// Define the struct associated with "offer_asset_info"
pub struct OfferAssetInfo {
    native_token: NativeToken,
}

impl OfferAssetInfo {
    pub fn new(native_token: NativeToken) -> Self {
        OfferAssetInfo {
            native_token: native_token,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UsdtContractAddr {
    contract_addr: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AskAssetInfo {
    pub token: UsdtContractAddr,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
// Define the struct represented by the "orai_swap" key
pub struct OraiSwap {
    offer_asset_info: OfferAssetInfo,
    ask_asset_info: AskAssetInfo,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Operation {
    pub orai_swap: OraiSwap,
}

impl Clone for Operation {
    fn clone(&self) -> Operation {
        Operation {
            orai_swap: self.orai_swap.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SwapContractMessageContent {
    pub offer_amount: u128,
    pub operations: Vec<Operation>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SwapContractMessage {
    pub simulate_swap_operations: SwapContractMessageContent,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Amount {
    amount: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ExchangeRateResponse {
    pub amount: u128,
}
