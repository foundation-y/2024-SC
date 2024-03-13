#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins,
    to_json_binary,
    BankMsg,
    Binary,
    CosmosMsg,
    Deps,
    DepsMut,
    Env,
    MessageInfo,
    Reply,
    Response,
    StdResult,
    SubMsg,
    SubMsgResult,
    Uint128,
    WasmMsg,
};

use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg,
    ExecuteResponse,
    InitMsg,
    ContractStatus,
    ResponseStatus,
    PaymentMethod,
    QueryMsg,
    QueryResponse,
};
use crate::state::{
    Config,
    Ido,
    Purchase,
    PURCHASES,
    IDO_TO_INFO,
    ACTIVE_IDOS,
    ARCHIVED_PURCHASES,
    CONFIG_KEY,
};
use crate::tier::{ get_min_tier, get_tier, get_tier_from_nft_contract };
use crate::utils::{ self, assert_admin, assert_contract_active, assert_ido_admin };
use cosmwasm_std::StdError;

pub const BLOCK_SIZE: usize = 256;
pub const ORAI: &str = "orai";
pub const WITHDRAW_TOKEN_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InitMsg
) -> Result<Response, ContractError> {
    let admin = msg.admin.unwrap_or(_info.sender.to_string());
    let canonical_admin = admin.to_string();
    let tier_contract = msg.tier_contract.to_string();
    let nft_contract = msg.nft_contract.to_string();
    let lock_periods_len = msg.lock_periods.len();

    let mut config = Config {
        admin: canonical_admin,
        status: ContractStatus::Active as u8,
        tier_contract,
        nft_contract,
        lock_periods: msg.lock_periods,
        min_tier: 0,
    };

    let min_tier = get_min_tier(&deps.as_ref(), &config)?;
    config.min_tier = min_tier;

    if lock_periods_len != (min_tier as usize) {
        return Err(
            ContractError::Std(
                StdError::generic_err(&format!("Lock periods array must have {} items", min_tier))
            )
        );
    }

    CONFIG_KEY.save(deps.storage, &config)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg
) -> Result<Response, ContractError> {
    let response = match msg {
        ExecuteMsg::ChangeAdmin { admin, .. } => change_admin(deps, env, info, admin),
        ExecuteMsg::ChangeStatus { status, .. } => change_status(deps, env, info, status),
        ExecuteMsg::StartIdo {
            start_time,
            end_time,
            token_contract: token_contract_addr,
            price,
            total_amount,
            soft_cap,
            tokens_per_tier,
            payment,
            boycott_disabled,
            ..
        } => {
            let mut ido = Ido::default();
            assert_admin(&deps, &info.sender.to_string())?;
            let admin = info.sender.to_string();
            let token_contract = token_contract_addr.to_string();
            ido.admin = admin;
            ido.start_time = start_time;
            ido.end_time = end_time;
            ido.token_contract = token_contract;
            ido.price = price.u128();
            ido.total_tokens_amount = total_amount.u128();
            ido.soft_cap = soft_cap.u128();
            ido.remaining_tokens_per_tier = tokens_per_tier
                .into_iter()
                .map(|v| v.u128())
                .collect();

            if let PaymentMethod::Token { contract, code_hash } = payment {
                let payment_token_contract = contract.to_string();
                ido.payment_token_contract = Some(payment_token_contract);
                ido.payment_token_hash = Some(code_hash);
            }

            ido.boycott_disabled = boycott_disabled;

            start_ido(deps, env, info, ido)
        }
        ExecuteMsg::BuyTokens { amount, ido_id, .. } =>
            buy_tokens(deps, env, info, ido_id, amount.u128()),
        ExecuteMsg::RecvTokens { ido_id, start, limit, purchase_indices, .. } =>
            recv_tokens(deps, env, info, ido_id, start, limit, purchase_indices),
        ExecuteMsg::Withdraw { ido_id, .. } => withdraw(deps, env, info, ido_id),
        ExecuteMsg::BoycottIdo { ido_id } => boycott_ido(deps, env, info, ido_id),
    };

    return response;
}

fn change_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    admin: String
) -> Result<Response, ContractError> {
    // Validate new admin address
    let validated = deps.api.addr_validate(&admin).unwrap();
    assert_eq!(validated, admin);

    assert_admin(&deps, &info.sender.to_string())?;

    let mut config = Config::load(deps.storage)?;
    let new_admin = admin.to_string();
    config.admin = new_admin;

    config.save(deps.storage)?;

    Ok(Response::new().add_attribute("action", "changed admin"))
}

fn change_status(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    status: ContractStatus
) -> Result<Response, ContractError> {
    assert_admin(&deps, &info.sender.to_string())?;

    let mut config = Config::load(deps.storage)?;
    config.status = status as u8;
    config.save(deps.storage)?;

    Ok(Response::new().add_attribute("action", "changed status"))
}

fn start_ido(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut ido: Ido
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    assert_admin(&deps, &info.sender.to_string())?;
    let config = Config::load(deps.storage)?;
    if ido.remaining_tokens_per_tier.len() != (config.min_tier as usize) {
        return Err(ContractError::Std(StdError::generic_err("`tokens_per_tier` has wrong size")));
    }

    let sum = ido.remaining_tokens_per_tier.iter().sum::<u128>();
    if sum < ido.total_tokens_amount {
        return Err(
            ContractError::Std(
                StdError::generic_err(
                    "Sum of `tokens_per_tier` can't be less than total tokens amount"
                )
            )
        );
    }

    if ido.start_time >= ido.end_time {
        return Err(
            ContractError::Std(StdError::generic_err("End time must be greater than start time"))
        );
    }

    if ido.price == 0 {
        return Err(ContractError::Std(StdError::generic_err("Ido price should not be ZERO!")));
    }
    if env.block.time.seconds() >= ido.end_time {
        return Err(ContractError::Std(StdError::generic_err("Ido ends in the past")));
    }

    if ido.soft_cap == 0 {
        return Err(ContractError::Std(StdError::generic_err("soft_cap should not be ZERO!")));
    }

    if ido.soft_cap > ido.total_tokens_amount {
        return Err(
            ContractError::Std(StdError::generic_err("soft_cap should be less than total amount"))
        );
    }

    let ido_id = ido.save(deps.storage)?;

    ido.save(deps.storage)?;

    let token_address = ido.token_contract.to_string();
    let transfer_msg = Cw20ExecuteMsg::TransferFrom {
        owner: info.sender.to_string(),
        recipient: env.contract.address.to_string(),
        amount: Uint128::new(ido.total_tokens_amount),
    };

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: token_address,
        msg: to_json_binary(&transfer_msg)?,
        funds: vec![],
    });

    let answer = to_json_binary(
        &(ExecuteResponse::StartIdo {
            ido_id,
            status: ResponseStatus::Success,
        })
    )?;

    Ok(Response::new().set_data(answer).add_submessage(sub_msg))
}

fn buy_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ido_id: u32,
    mut amount: u128
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;

    let sender = info.sender.to_string();
    let canonical_sender = sender.to_string();

    let mut ido = Ido::load(deps.storage, ido_id)?;
    if !ido.is_active(env.block.time.seconds()) {
        return Err(
            ContractError::Std(
                StdError::generic_err(format!("IDO is not active {}", env.block.time))
            )
        );
    }

    if ido.is_native_payment() {
        let orai_amount = utils::sent_funds(&info.funds)?;
        amount = orai_amount.checked_mul(ido.price).unwrap();
    }

    if amount == 0 {
        return Err(ContractError::Std(StdError::generic_err(format!("Zero amount {}", ido.id()))));
    }

    let config = Config::load(deps.storage)?;
    let tier = get_tier(&deps.as_ref(), sender.clone())?;

    let remaining_amount = ido.remaining_tokens_per_tier(tier);
    if remaining_amount == 0 {
        if ido.total_tokens_amount == ido.sold_amount {
            return Err(ContractError::Std(StdError::generic_err("All tokens are sold")));
        } else {
            return Err(
                ContractError::Std(StdError::generic_err("All tokens are sold for your tier"))
            );
        }
    }

    if amount > remaining_amount {
        let msg = format!("You cannot buy more than {} tokens", remaining_amount);
        return Err(ContractError::Std(StdError::generic_err(&msg)));
    }

    let payment = amount.checked_div(ido.price).unwrap();
    let lock_period = config.lock_period(tier);

    let unlock_time = ido.end_time.checked_add(lock_period).unwrap();
    let tokens_amount = Uint128::new(amount);
    let purchase = Purchase {
        timestamp: env.block.time.seconds(),
        tokens_amount: tokens_amount.u128(),
        unlock_time,
    };

    let mut purchases = PURCHASES.may_load(deps.storage, (
        canonical_sender.to_string(),
        ido_id,
    ))?.unwrap_or_default();
    purchases.push(purchase);
    PURCHASES.save(deps.storage, (canonical_sender.to_string(), ido_id), &purchases)?;

    let mut user_ido_info = IDO_TO_INFO.may_load(deps.storage, (
        canonical_sender.to_string(),
        ido_id,
    ))?.unwrap_or_default();

    if user_ido_info.total_payment == 0 {
        ido.participants = ido.participants.checked_add(1).unwrap();
    }

    user_ido_info.total_payment = user_ido_info.total_payment.checked_add(payment).unwrap();
    user_ido_info.total_tokens_bought = user_ido_info.total_tokens_bought
        .checked_add(amount)
        .unwrap();

    IDO_TO_INFO.save(deps.storage, (canonical_sender.to_string(), ido_id), &user_ido_info)?;

    ACTIVE_IDOS.save(deps.storage, (canonical_sender.to_string(), ido_id), &true)?;

    ido.sold_amount = ido.sold_amount.checked_add(amount).unwrap();
    ido.total_payment = ido.total_payment.checked_add(payment).unwrap();

    let tier_index = tier.checked_sub(1).unwrap() as usize;
    ido.remaining_tokens_per_tier[tier_index] = ido.remaining_tokens_per_tier[tier_index]
        .checked_sub(amount)
        .unwrap();

    ido.save(deps.storage)?;

    let answer = to_json_binary(
        &(ExecuteResponse::BuyTokens {
            unlock_time,
            amount: Uint128::new(amount),
            status: ResponseStatus::Success,
        })
    )?;

    if !ido.is_native_payment() {
        let token_contract_canonical = ido.payment_token_contract.unwrap();
        // let token_contract_hash = ido.payment_token_hash.unwrap();
        let token_contract = token_contract_canonical.to_string();

        let transfer_msg = Cw20ExecuteMsg::TransferFrom {
            owner: info.sender.to_string(),
            recipient: env.contract.address.to_string(),
            amount: Uint128::new(payment),
        };

        let sub_msg = SubMsg::new(WasmMsg::Execute {
            contract_addr: token_contract,
            msg: to_json_binary(&transfer_msg)?,
            funds: vec![],
        });

        return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
    }
    Ok(Response::new().set_data(answer))
}

fn recv_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ido_id: u32,
    start: Option<u32>,
    limit: Option<u32>,
    purchase_indices: Option<Vec<u32>>
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    //
    let canonical_sender = info.sender.to_string();
    let current_time = env.block.time;

    let ido = Ido::load(deps.storage, ido_id)?;
    let mut user_ido_info = IDO_TO_INFO.may_load(deps.storage, (
        canonical_sender.to_string(),
        ido_id,
    ))?.unwrap_or_default();

    // when ido failed, withdraw the payment tokens.
    if current_time.seconds() > ido.end_time && ido.soft_cap > ido.sold_amount {
        let refund_amount = user_ido_info.total_payment;
        user_ido_info.total_tokens_received = 0;
        user_ido_info.total_tokens_bought = 0;
        user_ido_info.total_payment = 0;

        IDO_TO_INFO.save(deps.storage, (canonical_sender.to_string(), ido_id), &user_ido_info)?;
        ACTIVE_IDOS.remove(deps.storage, (canonical_sender.to_string(), ido_id));

        let answer = to_json_binary(
            &(ExecuteResponse::RecvTokens {
                amount: Uint128::new(user_ido_info.total_payment),
                status: ResponseStatus::Success,
                ido_success: false,
            })
        )?;

        if ido.is_native_payment() {
            let transfer_msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(refund_amount, ORAI),
            });
            return Ok(Response::new().set_data(answer).add_message(transfer_msg));
        } else {
            let token_contract_canonical = ido.payment_token_contract.unwrap();
            // let token_contract_hash = ido.payment_token_hash.unwrap();
            let token_contract = token_contract_canonical.to_string();

            let transfer_msg = Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.to_string(),
                amount: Uint128::new(user_ido_info.total_payment),
            };

            let sub_msg = SubMsg::new(WasmMsg::Execute {
                contract_addr: token_contract,
                msg: to_json_binary(&transfer_msg)?,
                funds: vec![],
            });
            return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
        }
    }
    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(300);
    let mut purchases = PURCHASES.may_load(deps.storage, (
        canonical_sender.to_string(),
        ido_id,
    ))?.unwrap_or_default();
    let purchases_iter = purchases
        .iter()
        .skip(start as usize)
        .take(limit as usize);

    let mut indices = Vec::new();
    for (i, purchase) in purchases_iter.enumerate() {
        if current_time.seconds() >= purchase.unlock_time {
            let index = i.checked_add(start as usize).unwrap();
            indices.push(index);
        }
    }

    if let Some(purchase_indices) = purchase_indices {
        let end = start.checked_add(limit).unwrap();
        for index in purchase_indices {
            if index >= start && index < end {
                continue;
            }

            let purchase = purchases.get(index as usize).unwrap();
            if current_time.seconds() >= purchase.unlock_time {
                indices.push(index as usize);
            }
        }
    }

    indices.sort();
    indices.dedup();

    let mut recv_amount: u128 = 0;

    let mut archived_purchases = ARCHIVED_PURCHASES.may_load(deps.storage, (
        canonical_sender.to_string(),
        ido_id,
    ))?.unwrap_or_default();

    for (shift, index) in indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        let purchase = purchases.remove(position as usize);

        recv_amount = recv_amount.checked_add(purchase.tokens_amount).unwrap();
        archived_purchases.push(purchase);
    }
    PURCHASES.save(deps.storage, (canonical_sender.to_string(), ido_id), &purchases)?;
    ARCHIVED_PURCHASES.save(
        deps.storage,
        (canonical_sender.to_string(), ido_id),
        &archived_purchases
    )?;

    if recv_amount == 0 {
        return Err(ContractError::Std(StdError::generic_err("Nothing to receive")));
    }

    let answer = to_json_binary(
        &(ExecuteResponse::RecvTokens {
            amount: Uint128::new(recv_amount),
            status: ResponseStatus::Success,
            ido_success: true,
        })
    )?;

    user_ido_info.total_tokens_received += recv_amount;

    IDO_TO_INFO.save(deps.storage, (canonical_sender.to_string(), ido_id), &user_ido_info)?;

    if user_ido_info.total_tokens_bought == user_ido_info.total_tokens_received {
        ACTIVE_IDOS.remove(deps.storage, (canonical_sender.to_string(), ido_id));
    }

    let token_contract = ido.token_contract.to_string();

    let transfer_msg = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.to_string(),
        amount: Uint128::new(recv_amount),
    };

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: token_contract,
        msg: to_json_binary(&transfer_msg)?,
        funds: vec![],
    });
    return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
}

fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ido_id: u32
) -> Result<Response, ContractError> {
    let ido_admin = info.sender.to_string();
    assert_ido_admin(&deps, &ido_admin, ido_id)?;
    assert_contract_active(deps.storage)?;

    let mut ido = Ido::load(deps.storage, ido_id)?;
    if ido.withdrawn {
        return Err(ContractError::Std(StdError::generic_err("Already withdrawn")));
    }

    if env.block.time.seconds() < ido.end_time {
        return Err(ContractError::Std(StdError::generic_err("IDO is not finished yet")));
    }

    ido.withdrawn = true;
    ido.save(deps.storage)?;

    let remaining_tokens: Uint128;
    if ido.soft_cap > ido.sold_amount {
        remaining_tokens = Uint128::from(ido.total_tokens_amount);
    } else {
        remaining_tokens = Uint128::from(ido.remaining_tokens());
    }

    let ido_token_contract = ido.token_contract.to_string();

    let mut msgs = vec![];
    let mut submsgs = vec![];
    if !remaining_tokens.is_zero() {
        let transfer_msg = Cw20ExecuteMsg::TransferFrom {
            owner: env.contract.address.to_string(),
            recipient: ido_admin.to_string(),
            amount: remaining_tokens,
        };

        let sub_msg = SubMsg::new(WasmMsg::Execute {
            contract_addr: ido_token_contract,
            msg: to_json_binary(&transfer_msg)?,
            funds: vec![],
        });

        submsgs.push(sub_msg);
    }
    //withdraw payment tokens.
    let payment_amount = Uint128::new(ido.sold_amount.checked_div(ido.price).unwrap());
    if ido.sold_amount >= ido.soft_cap {
        if ido.is_native_payment() {
            msgs.push(
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: ido_admin,
                    amount: coins(ido.sold_amount.checked_div(ido.price).unwrap(), ORAI),
                })
            );
        } else {
            let token_contract_canonical = ido.payment_token_contract.unwrap();
            // let token_contract_hash = ido.payment_token_hash.unwrap();
            let token_contract = token_contract_canonical.to_string();

            let transfer_msg = Cw20ExecuteMsg::TransferFrom {
                owner: env.contract.address.to_string(),
                recipient: ido_admin.to_string(),
                amount: payment_amount,
            };

            let sub_msg = SubMsg::new(WasmMsg::Execute {
                contract_addr: token_contract,
                msg: to_json_binary(&transfer_msg)?,
                funds: vec![],
            });

            submsgs.push(sub_msg);
        }
    }

    let answer = to_json_binary(
        &(ExecuteResponse::Withdraw {
            ido_amount: remaining_tokens,
            payment_amount: payment_amount,
            status: ResponseStatus::Success,
        })
    )?;

    return Ok(Response::new().set_data(answer).add_messages(msgs).add_submessages(submsgs));
}

pub fn boycott_ido(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ido_id: u32
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;

    let canonical_sender = info.sender.to_string();
    let current_time = env.block.time;

    let ido = Ido::load(deps.storage, ido_id)?;

    // Check if the startup disabled this function
    if ido.boycott_disabled {
        return Err(
            ContractError::Std(StdError::generic_err("This function is disabled by startup."))
        );
    }
    // Check if the user is trying to decline in a day after IDO ended.
    if ido.end_time > current_time.seconds() || current_time.seconds() > ido.end_time + 86400 {
        return Err(
            ContractError::Std(
                StdError::generic_err(
                    format!(
                        "A user cannot withdraw before the IDO ends, nor can they do so after 24 hours has passed since the end of the IDO."
                    )
                )
            )
        );
    }

    let mut msgs = vec![];
    let mut submsgs = vec![];

    let mut user_ido_info = IDO_TO_INFO.may_load(deps.storage, (
        canonical_sender.to_string(),
        ido_id,
    ))?.unwrap_or_default();

    // Withdraw received tokens
    let withdraw_token_amount: Uint128 = user_ido_info.total_tokens_received.into();
    if user_ido_info.total_tokens_received != 0 {
        let ido_token_contract = ido.token_contract.to_string();
        let transfer_msg = Cw20ExecuteMsg::TransferFrom {
            owner: info.sender.to_string(),
            recipient: env.contract.address.to_string(),
            amount: withdraw_token_amount,
        };

        let msg = WasmMsg::Execute {
            contract_addr: ido_token_contract,
            msg: to_json_binary(&transfer_msg)?,
            funds: vec![],
        };

        let sub_msg = SubMsg::reply_always(msg, 1);

        submsgs.push(sub_msg);
    }

    // Refund payment from contract to user
    let refund_payment = user_ido_info.total_payment;
    if ido.is_native_payment() {
        msgs.push(
            CosmosMsg::Bank(BankMsg::Send {
                to_address: canonical_sender,
                amount: coins(refund_payment, ORAI),
            })
        );
    } else {
        let token_contract_canonical = ido.payment_token_contract.unwrap();
        // let token_contract_hash = ido.payment_token_hash.unwrap();
        let token_contract = token_contract_canonical.to_string();

        let transfer_msg = Cw20ExecuteMsg::Transfer {
            recipient: canonical_sender,
            amount: Uint128::new(refund_payment),
        };

        let sub_msg = SubMsg::new(WasmMsg::Execute {
            contract_addr: token_contract,
            msg: to_json_binary(&transfer_msg)?,
            funds: vec![],
        });
        submsgs.push(sub_msg);
    }

    // Reset the user's ido info and Ido info
    let mut ido = Ido::load(deps.storage, ido_id)?;
    ido.participants = ido.participants.checked_sub(1).unwrap_or_default();
    ido.sold_amount = ido.sold_amount
        .checked_sub(user_ido_info.total_tokens_bought)
        .unwrap_or_default();
    ido.total_payment = ido.total_payment.checked_sub(refund_payment).unwrap_or_default();
    let tier = get_tier(&deps.as_ref(), info.sender.to_string())?;
    let tier_index = tier.checked_sub(1).unwrap() as usize;
    ido.remaining_tokens_per_tier[tier_index] = ido.remaining_tokens_per_tier[tier_index]
        .checked_add(user_ido_info.total_tokens_bought)
        .unwrap();

    user_ido_info.total_tokens_received = 0;
    user_ido_info.total_tokens_bought = 0;
    user_ido_info.total_payment = 0;

    ido.save(deps.storage)?;

    IDO_TO_INFO.save(deps.storage, (info.sender.to_string(), ido_id), &user_ido_info)?;

    let answer = to_json_binary(
        &(ExecuteResponse::BoycottIdo {
            token_amount: withdraw_token_amount,
            payment_amount: refund_payment.into(),
        })
    )?;

    return Ok(Response::new().set_data(answer).add_messages(msgs).add_submessages(submsgs));
}

#[entry_point]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id == WITHDRAW_TOKEN_REPLY_ID {
        match msg.result {
            SubMsgResult::Ok(_) => {
                // Perform actions upon successful token withdrawal
                Ok(Response::new().add_attribute("boycott_withdraw_token", "success"))
            }
            SubMsgResult::Err(_err) => {
                // Perform actions upon failed token withdrawal
                return Err(
                    ContractError::Std(
                        StdError::generic_err(
                            &format!(
                                "The contract does not have Allowance from the user to move its tokens"
                            )
                        )
                    )
                );
            }
        }
    } else {
        Ok(Response::new())
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let response = match msg {
        QueryMsg::Config {} => {
            let config = Config::load(deps.storage)?;
            config.to_answer()?
        }
        QueryMsg::IdoAmount {} => {
            let amount = Ido::len(deps.storage)?;
            QueryResponse::IdoAmount { amount }
        }
        QueryMsg::IdoInfo { ido_id } => {
            let ido = Ido::load(deps.storage, ido_id)?;
            ido.to_answer()?
        }
        QueryMsg::Purchases { ido_id, address, start, limit } => {
            let canonical_address = address.clone();

            let purchases = PURCHASES.may_load(deps.storage, (
                canonical_address.to_string(),
                ido_id,
            ))?.unwrap_or_default();
            let amount = purchases.len() as u32;

            let start = start.unwrap_or(0);
            let limit = limit.unwrap_or(300);

            let mut raw_purchases: Vec<Purchase> = Vec::new();
            for i in start..start + limit {
                if i < amount {
                    raw_purchases.push(
                        purchases
                            .get(i as usize)
                            .unwrap()
                            .clone()
                    );
                }
            }

            let purchases = raw_purchases
                .into_iter()
                .map(|p| p.to_answer())
                .collect();

            QueryResponse::Purchases { purchases, amount }
        }
        QueryMsg::ArchivedPurchases { ido_id, address, start, limit } => {
            let canonical_address = address.clone();
            let purchases = ARCHIVED_PURCHASES.may_load(deps.storage, (
                canonical_address.to_string(),
                ido_id,
            ))?.unwrap_or_default();
            let amount = purchases.len() as u32;

            let mut raw_purchases: Vec<Purchase> = Vec::new();
            for i in start..start + limit {
                if i < amount {
                    raw_purchases.push(
                        purchases
                            .get(i as usize)
                            .unwrap()
                            .clone()
                    );
                }
            }

            let purchases = raw_purchases
                .into_iter()
                .map(|p| p.to_answer())
                .collect();

            QueryResponse::ArchivedPurchases { purchases, amount }
        }
        QueryMsg::UserInfo { address, ido_id } => {
            let canonical_address = address.clone();

            let user_info = IDO_TO_INFO.may_load(deps.storage, (
                canonical_address.to_string(),
                ido_id,
            ))?.unwrap_or_default();

            user_info.to_answer()
        }
        QueryMsg::TierInfo { address } => {
            let tier = get_tier(&deps, address.clone())?;
            let config = Config::load(deps.storage)?;
            let from_nft_contract = get_tier_from_nft_contract(&deps, &address, &config).unwrap();
            let mut nft_tier = 5;
            if let Some(value) = from_nft_contract {
                nft_tier = value;
            }
            QueryResponse::TierInfo { tier, nft_tier }
        }
    };
    to_json_binary(&response)
}
