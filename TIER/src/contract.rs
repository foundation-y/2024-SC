#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin,
    coins,
    to_json_binary,
    BankMsg,
    Binary,
    Coin,
    CosmosMsg,
    Deps,
    DepsMut,
    Env,
    FullDelegation,
    MessageInfo,
    Response,
    StdResult,
    SubMsg,
    Uint128,
};

use cosmwasm_std::DistributionMsg;
use cosmwasm_std::StakingMsg;

use crate::band::OraiPriceOracle;
// use crate::utils;
use crate::error::ContractError;
use crate::msg::{
    self,
    ContractStatus,
    ExecuteMsg,
    ExecuteResponse,
    InstantiateMsg,
    OraiswapContract,
    QueryMsg,
    QueryResponse,
    ResponseStatus,
    SerializedUnbonds,
    SerializedWithdrawals,
    ValidatorWithWeight,
};
use crate::state::{
    self,
    Config,
    UserUnbond,
    UserWithdrawal,
    CONFIG_ITEM,
    UNBOND_LIST,
    USER_INFOS,
    USER_TOTAL_DELEGATED,
    WITHDRAWALS_LIST,
};
use crate::utils;
use cosmwasm_std::StdError;

pub const UNBOUND_TIME: u64 = 21 * 24 * 60 * 60;
pub const BATCH_PERIOD: u64 = 5 * 24 * 60 * 60;
pub const MAX_UNIX_TIMESTAMP: u64 = 2147483647;
pub const ORAI: &str = "orai";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg
) -> Result<Response, ContractError> {
    let deposits = msg.deposits
        .iter()
        .map(|v| v.u128())
        .collect::<Vec<_>>();

    if deposits.is_empty() {
        return Err(ContractError::Std(StdError::generic_err("Deposits array is empty")));
    }

    let is_sorted = deposits
        .as_slice()
        .windows(2)
        .all(|v| v[0] > v[1]);
    if !is_sorted {
        return Err(
            ContractError::Std(StdError::generic_err("Specify deposits in decreasing order"))
        );
    }

    // Check if the sum of the validators' weights is 100
    let validators = msg.validators;
    let total_weight: u128 = validators
        .iter()
        .map(|v| v.weight)
        .sum();

    if total_weight != 100 {
        return Err(
            ContractError::Std(StdError::generic_err("The sum of the total weight must be 100!"))
        );
    }

    let admin = msg.admin.unwrap_or("".to_string());
    let initial_config: Config = Config {
        status: ContractStatus::Active as u8,
        admin: admin,
        validators,
        usd_deposits: deposits,
        oraiswap_contract: msg.oraiswap_contract,
        stable_denom: msg.stable_denom.unwrap_or_default(),
    };

    CONFIG_ITEM.save(deps.storage, &initial_config)?;
    // initial_config.save(&deps.storage)?;

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
        ExecuteMsg::ChangeAdmin { admin, .. } => try_change_admin(deps, env, info, admin),
        ExecuteMsg::ChangeStatus { status, .. } => try_change_status(deps, env, info, status),
        ExecuteMsg::ChangeOraiswap { oraiswap_router_contract, usdt_contract } =>
            try_change_oraiswap(deps, env, info, oraiswap_router_contract, usdt_contract),
        ExecuteMsg::Deposit { .. } => try_deposit(deps, env, info),
        ExecuteMsg::Withdraw { .. } => try_withdraw(deps, env, info),
        ExecuteMsg::BatchUnbond { .. } => try_batch_unbond(deps, env),
        ExecuteMsg::Claim { recipient, start, limit, .. } =>
            try_claim(deps, env, info, recipient, start, limit),
        ExecuteMsg::WithdrawRewards { recipient, .. } => {
            try_withdraw_rewards(deps, env, info, recipient)
        }
        ExecuteMsg::Redelegate {
            new_validator_address,
            old_validator_address,
            delegate_ratio,
            recipient,
            ..
        } =>
            try_redelegate(
                deps,
                env,
                info,
                new_validator_address,
                old_validator_address,
                delegate_ratio,
                recipient
            ),
    };

    return response;
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::UserInfo { address } => to_json_binary(&query_user_info(deps, address)?),
        QueryMsg::UserTotalDelegated { address } =>
            to_json_binary(&query_user_total_delegated(deps, address)?),
        QueryMsg::Withdrawals { address, start, limit } =>
            to_json_binary(&query_withdrawals(deps, address, start, limit)?),
        QueryMsg::Unbonds {} => to_json_binary(&query_unbonds(deps)?),
    }
}

pub fn try_change_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_admin: String
) -> Result<Response, ContractError> {
    let config: Config = CONFIG_ITEM.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }

    CONFIG_ITEM.update(
        deps.storage,
        |mut exists| -> StdResult<_> {
            exists.admin = new_admin;
            Ok(exists)
        }
    )?;

    Ok(Response::new().add_attribute("action", "changed admin"))
}

pub fn try_change_status(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    status: ContractStatus
) -> Result<Response, ContractError> {
    let config: Config = CONFIG_ITEM.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }

    // Check the status is not set to the same value
    if status == config.status.into() {
        return Err(
            ContractError::Std(
                StdError::generic_err("Trying to change the status to the same value...")
            )
        );
    } else {
        CONFIG_ITEM.update(
            deps.storage,
            |mut exists| -> StdResult<_> {
                exists.status = status as u8;
                Ok(exists)
            }
        )?;
    }

    Ok(Response::new().add_attribute("action", "changed status"))
}

pub fn try_change_oraiswap(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    oraiswap_router_contract: String,
    usdt_contract: String
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG_ITEM.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }

    // Validate Oraiswap contracts' addresses
    let validated_oraiswap_router = deps.api.addr_validate(&oraiswap_router_contract).unwrap();
    assert_eq!(validated_oraiswap_router, oraiswap_router_contract);
    let validated_usdt = deps.api.addr_validate(&usdt_contract).unwrap();
    assert_eq!(validated_usdt, usdt_contract);

    // Change Oraiswap contracts
    if
        config.oraiswap_contract.orai_swap_router_contract == oraiswap_router_contract &&
        config.oraiswap_contract.usdt_contract == usdt_contract
    {
        return Err(
            ContractError::Std(StdError::generic_err("Trying to change to the same addresses."))
        );
    } else {
        config.oraiswap_contract = OraiswapContract {
            orai_swap_router_contract: oraiswap_router_contract,
            usdt_contract,
        };
        config.save(deps.storage)?;
    }

    Ok(Response::new().add_attribute("action", "changed oraiswap contracts"))
}

pub fn get_received_funds(_deps: &DepsMut, info: &MessageInfo) -> Result<Coin, ContractError> {
    let config = CONFIG_ITEM.load(_deps.storage)?;
    config.assert_contract_active()?;

    match info.funds.get(0) {
        None => {
            return Err(ContractError::Std(StdError::generic_err("No Funds")));
        }
        Some(received) => {
            /* Amount of tokens received cannot be zero */
            if received.amount.is_zero() {
                return Err(ContractError::Std(StdError::generic_err("Not Allow Zero Amount")));
            }

            /* Allow to receive only token denomination defined
            on contract instantiation "config.stable_denom" */
            if
                received.denom.clone() != "orai" &&
                config.stable_denom.contains(&received.denom.clone()) == false
            {
                return Err(ContractError::Std(StdError::generic_err("Unsopported token")));
            }

            /* Only one token can be received */
            if info.funds.len() > 1 {
                return Err(ContractError::Std(StdError::generic_err("Not Allowed Multiple Funds")));
            }
            Ok(received.clone())
        }
    }
}

pub fn try_deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    config.assert_contract_active()?;

    let received_funds = get_received_funds(&deps, &info)?;

    let orai_deposit = received_funds.amount.u128();

    let orai_price_ocracle = OraiPriceOracle::new(&deps)?;

    let usd_deposit: u128 = orai_price_ocracle.usd_amount(orai_deposit);

    let sender = info.sender.to_string();
    let min_tier = config.min_tier();

    let mut user_info = USER_INFOS.may_load(deps.storage, sender)?.unwrap_or(state::UserInfo {
        tier: min_tier,
        ..Default::default()
    });
    let current_tier = user_info.tier;
    let old_usd_deposit = user_info.usd_deposit;
    let old_orai_deposit = user_info.orai_deposit;
    let new_usd_deposit = old_usd_deposit.checked_add(usd_deposit).unwrap();

    let new_tier = config.tier_by_deposit(new_usd_deposit);

    if current_tier == new_tier {
        if current_tier == config.max_tier() {
            return Err(ContractError::Std(StdError::generic_err("Reached max tier")));
        }

        let next_tier = current_tier.checked_sub(1).unwrap();
        let next_tier_deposit: u128 = config.deposit_by_tier(next_tier);

        let expected_deposit_usd = next_tier_deposit.checked_sub(old_usd_deposit).unwrap();
        let expected_deposit_orai = orai_price_ocracle.orai_amount(expected_deposit_usd);

        let err_msg = format!(
            "You should deposit at least {} USD ({} ORAI)",
            expected_deposit_usd,
            expected_deposit_orai
        );

        return Err(ContractError::Std(StdError::generic_err(&err_msg)));
    }

    let mut messages: Vec<SubMsg> = Vec::with_capacity(2);
    let new_tier_deposit = config.deposit_by_tier(new_tier);

    // let usd_refund = new_usd_deposit.checked_sub(new_tier_deposit).unwrap();
    let orai_refund = orai_deposit
        .checked_sub(orai_price_ocracle.orai_amount(new_tier_deposit - old_usd_deposit))
        .unwrap();

    if orai_refund != 0 {
        // orai_deposit = orai_deposit.checked_sub(orai_refund).unwrap();

        let send_msg = BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(orai_refund, ORAI),
        };

        let msg = CosmosMsg::Bank(send_msg);
        messages.push(SubMsg::new(msg));
    }
    let old_orai_deposit = user_info.orai_deposit;
    user_info.tier = new_tier;
    user_info.timestamp = env.block.time.seconds();
    user_info.usd_deposit = new_tier_deposit;
    // user_info.orai_deposit = user_info.orai_deposit.checked_add(orai_deposit).unwrap();
    let orai_deposit = orai_price_ocracle.orai_amount(user_info.usd_deposit);
    user_info.orai_deposit = orai_deposit;

    // Calculate user's total delegated amount
    let mut user_total_delegated = USER_TOTAL_DELEGATED.may_load(
        deps.storage,
        info.sender.to_string()
    )?.unwrap_or_default();

    user_total_delegated = user_total_delegated
        .checked_add(Uint128::from(orai_deposit))
        .unwrap()
        .checked_sub(Uint128::from(old_orai_deposit))
        .unwrap();

    USER_TOTAL_DELEGATED.save(deps.storage, info.sender.to_string(), &user_total_delegated)?;
    //////////////////////////////////////////

    USER_INFOS.save(deps.storage, info.sender.to_string(), &user_info)?;

    let validators = config.validators;

    for validator in validators {
        let individual_amount =
            (user_info.orai_deposit.checked_sub(old_orai_deposit).unwrap() * validator.weight) /
            100;
        let delegate_msg = StakingMsg::Delegate {
            validator: validator.address,
            amount: coin(individual_amount, ORAI),
        };

        let msg: CosmosMsg = CosmosMsg::Staking(delegate_msg);
        messages.push(SubMsg::new(msg));
    }

    let answer = to_json_binary(
        &(ExecuteResponse::Deposit {
            usd_deposit: Uint128::new(user_info.usd_deposit),
            orai_deposit: Uint128::new(user_info.orai_deposit),
            tier: new_tier,
            status: ResponseStatus::Success,
        })
    )?;

    Ok(Response::new().add_submessages(messages).set_data(answer))
}

pub fn try_withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let _env = env.clone();
    let config = CONFIG_ITEM.load(deps.storage)?;
    let contract_address = _env.contract.address;
    config.assert_contract_active()?;

    let sender = info.sender.to_string();

    let min_tier = config.min_tier();
    let user_info = USER_INFOS.may_load(deps.storage, sender)?.unwrap_or(state::UserInfo {
        tier: min_tier,
        ..Default::default()
    });

    let mut amount: u128 = user_info.orai_deposit;

    // Consider the validator slashing
    let mut total_delegated_with_slashing = 0;
    let mut total_staked = 0;

    // Get the total staked amount
    let delegated_iterator: Vec<_> = USER_TOTAL_DELEGATED.range(
        deps.storage,
        None,
        None,
        cosmwasm_std::Order::Ascending
    ).collect::<_>();

    for delegate in delegated_iterator {
        let temp = delegate.unwrap();
        total_staked += temp.1.u128();
    }

    //////////////////////////////////////////

    // Get total delegated amount from all validators considering with slashing
    for validator in config.validators.clone() {
        let current_delegate: Option<FullDelegation> = deps.querier
            .query_delegation(contract_address.clone(), validator.clone().address)
            .unwrap();

        if let Some(full_delegation) = current_delegate {
            let delegated_coin: Coin = full_delegation.amount;
            let delegated_amount = delegated_coin.amount.u128();
            total_delegated_with_slashing += delegated_amount;
        } else {
            let err_msg = format!("No delegation was found!");

            return Err(ContractError::Std(StdError::generic_err(&err_msg)));
        }
    }

    amount = amount
        .checked_mul(total_delegated_with_slashing)
        .unwrap()
        .checked_div(total_staked)
        .unwrap();

    USER_INFOS.remove(deps.storage, info.sender.to_string());

    let current_time = env.block.time.seconds();
    let claim_time = MAX_UNIX_TIMESTAMP;
    let withdrawal = UserWithdrawal {
        amount,
        timestamp: current_time,
        claim_time,
    };

    let mut withdrawals = WITHDRAWALS_LIST.may_load(
        deps.storage,
        info.sender.to_string()
    )?.unwrap_or_default();

    withdrawals.push(withdrawal);
    WITHDRAWALS_LIST.save(deps.storage, info.sender.to_string(), &withdrawals)?;

    let unbond_element = UserUnbond {
        address: info.sender.into(),
        amount,
        timestamp: current_time,
    };

    let _ = UNBOND_LIST.push_back(deps.storage, &unbond_element);

    // Batch Unbond whenever withdrawal happen

    if UNBOND_LIST.len(deps.storage)? == 0 {
        let err_msg = format!("Unbond List is Empty!");
        return Err(ContractError::Std(StdError::generic_err(&err_msg)));
    }

    let mut first_unbond = UNBOND_LIST.front(deps.storage)?.unwrap();

    if current_time - first_unbond.timestamp >= BATCH_PERIOD {
        // Get the total undelegated amount, Pop valid unbond action from Deque, Update the claim times
        let mut total_batch_undelegate_amount = 0;

        while UNBOND_LIST.len(deps.storage).unwrap() > 0 {
            first_unbond = UNBOND_LIST.front(deps.storage)?.unwrap();
            total_batch_undelegate_amount += first_unbond.amount;
            let key_address = first_unbond.address.to_string();
            let mut withdrawals = WITHDRAWALS_LIST.may_load(
                deps.storage,
                key_address.clone()
            )?.unwrap_or_default();

            // Calculate user's total delegated amount by subtracting undelegated amount
            let mut user_total_delegated = USER_TOTAL_DELEGATED.may_load(
                deps.storage,
                key_address.to_string()
            )?.unwrap_or_default();

            user_total_delegated = user_total_delegated
                .checked_sub(Uint128::from(first_unbond.amount))
                .unwrap();

            USER_TOTAL_DELEGATED.save(
                deps.storage,
                key_address.to_string(),
                &user_total_delegated
            )?;
            //////////////////////////////////////////

            for i in 0..withdrawals.len() {
                if
                    withdrawals[i].claim_time == MAX_UNIX_TIMESTAMP &&
                    withdrawals[i].amount == first_unbond.amount &&
                    withdrawals[i].timestamp == first_unbond.timestamp
                {
                    withdrawals[i].claim_time = current_time.checked_add(UNBOUND_TIME).unwrap();
                }
            }
            WITHDRAWALS_LIST.save(deps.storage, key_address.clone(), &withdrawals)?;

            // Pop
            UNBOND_LIST.pop_front(deps.storage)?;
        }

        let validators = config.validators;
        let confirmed_amount = total_batch_undelegate_amount.checked_sub(4).unwrap();

        let mut messages: Vec<SubMsg> = Vec::with_capacity(2);

        for validator in validators {
            let weight_as_uint128 = Uint128::from(validator.weight);

            // Perform the multiplication - Uint128 * Uint128
            let multiplied = Uint128::from(confirmed_amount).multiply_ratio(
                weight_as_uint128,
                Uint128::from(100_u128)
            );

            // Now, `multiplied` is Uint128, but we want the result as u128
            let individual_amount: u128 = multiplied.u128();

            let undelegate_msg = StakingMsg::Undelegate {
                validator: validator.address,
                amount: coin(individual_amount, ORAI),
            };
            let msg = CosmosMsg::Staking(undelegate_msg);
            messages.push(SubMsg::new(msg));
        }

        let answer = to_json_binary(
            &(ExecuteResponse::Withdraw {
                status: ResponseStatus::Success,
            })
        )?;
        Ok(
            Response::new()
                .add_submessages(messages)
                .set_data(answer)
                .add_attribute("action", "Add to withdraw list and batch unbond done!")
        )
    } else {
        Ok(Response::new().add_attribute("action", "Add to withdraw list!"))
    }
}

pub fn try_batch_unbond(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    if UNBOND_LIST.len(deps.storage)? == 0 {
        let err_msg = format!("Unbond List is Empty!");
        return Err(ContractError::Std(StdError::generic_err(&err_msg)));
    }

    let config = CONFIG_ITEM.load(deps.storage)?;
    let current_time = env.block.time.seconds();

    let mut first_unbond = UNBOND_LIST.front(deps.storage)?.unwrap();

    if current_time - first_unbond.timestamp < BATCH_PERIOD {
        let err_msg = format!("The time difference is lower than 5 days!");
        return Err(ContractError::Std(StdError::generic_err(&err_msg)));
    }

    // Get the total undelegated amount, Pop valid unbond action from Deque, Update the claim times
    let mut total_batch_undelegate_amount = 0;

    while UNBOND_LIST.len(deps.storage).unwrap() > 0 {
        first_unbond = UNBOND_LIST.front(deps.storage)?.unwrap();
        total_batch_undelegate_amount += first_unbond.amount;
        let key_address = first_unbond.address.to_string();
        let mut withdrawals = WITHDRAWALS_LIST.may_load(
            deps.storage,
            key_address.clone()
        )?.unwrap_or_default();

        // Calculate user's total delegated amount by subtracting undelegated amount
        let mut user_total_delegated = USER_TOTAL_DELEGATED.may_load(
            deps.storage,
            key_address.to_string()
        )?.unwrap_or_default();

        user_total_delegated = user_total_delegated
            .checked_sub(Uint128::from(first_unbond.amount))
            .unwrap();

        USER_TOTAL_DELEGATED.save(deps.storage, key_address.to_string(), &user_total_delegated)?;
        //////////////////////////////////////////

        for i in 0..withdrawals.len() {
            if
                withdrawals[i].claim_time == MAX_UNIX_TIMESTAMP &&
                withdrawals[i].amount == first_unbond.amount &&
                withdrawals[i].timestamp == first_unbond.timestamp
            {
                withdrawals[i].claim_time = current_time.checked_add(UNBOUND_TIME).unwrap();
            }
        }
        WITHDRAWALS_LIST.save(deps.storage, key_address.clone(), &withdrawals)?;

        // Pop
        UNBOND_LIST.pop_front(deps.storage)?;
    }

    let validators = config.validators;
    let confirmed_amount = total_batch_undelegate_amount.checked_sub(4).unwrap();

    let mut messages: Vec<SubMsg> = Vec::with_capacity(2);

    for validator in validators {
        let weight_as_uint128 = Uint128::from(validator.weight);

        // Perform the multiplication - Uint128 * Uint128
        let multiplied = Uint128::from(confirmed_amount).multiply_ratio(
            weight_as_uint128,
            Uint128::from(100_u128)
        );

        // Now, `multiplied` is Uint128, but we want the result as u128
        let individual_amount: u128 = multiplied.u128();

        let undelegate_msg = StakingMsg::Undelegate {
            validator: validator.address,
            amount: coin(individual_amount, ORAI),
        };
        let msg = CosmosMsg::Staking(undelegate_msg);
        messages.push(SubMsg::new(msg));
    }

    let answer = to_json_binary(
        &(ExecuteResponse::Withdraw {
            status: ResponseStatus::Success,
        })
    )?;

    Ok(Response::new().add_submessages(messages).set_data(answer))
}

pub fn try_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Option<String>,
    start: Option<u32>,
    limit: Option<u32>
) -> Result<Response, ContractError> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    config.assert_contract_active()?;

    let sender = info.sender.to_string();
    let mut withdrawals: Vec<UserWithdrawal> = WITHDRAWALS_LIST.may_load(
        deps.storage,
        sender
    )?.unwrap_or_default();

    let length = withdrawals.len();

    if length == 0 {
        return Err(ContractError::Std(StdError::generic_err("Nothing to claim")));
    }

    let recipient = recipient.unwrap_or(info.sender.to_string());
    let start: usize = start.unwrap_or(0) as usize;
    let limit = limit.unwrap_or(50) as usize;
    let withdrawals_iter: std::iter::Take<
        std::iter::Skip<std::slice::Iter<'_, UserWithdrawal>>
    > = withdrawals.iter().skip(start).take(limit);

    let current_time = env.block.time.seconds();
    let mut remove_indices = Vec::new();
    let mut claim_amount = 0u128;

    for (index, withdrawal) in withdrawals_iter.enumerate() {
        let claim_time = withdrawal.claim_time;

        if current_time >= claim_time {
            remove_indices.push(index);
            claim_amount = claim_amount.checked_add(withdrawal.amount).unwrap();
        }
    }

    if claim_amount == 0 {
        return Err(ContractError::Std(StdError::generic_err("Nothing to claim")));
    }

    for (shift, index) in remove_indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        withdrawals.remove(position);
    }

    let send_msg = BankMsg::Send {
        to_address: recipient,
        amount: coins(claim_amount, ORAI),
    };

    let msg = CosmosMsg::Bank(send_msg);
    let answer = to_json_binary(
        &(ExecuteResponse::Claim {
            amount: claim_amount.into(),
            status: ResponseStatus::Success,
        })
    )?;

    Ok(Response::new().add_message(msg).set_data(answer))
}

pub fn try_withdraw_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Option<String>
) -> Result<Response, ContractError> {
    let config: Config = CONFIG_ITEM.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }

    let admin = config.admin;
    let recipient = recipient.unwrap_or(admin);
    let mut msgs: Vec<CosmosMsg> = Vec::new();
    let set_withdraw_addr_msg = DistributionMsg::SetWithdrawAddress { address: recipient };
    msgs.push(CosmosMsg::Distribution(set_withdraw_addr_msg));

    let mut total_withdraw_amount: u128 = 0;

    let validators = &config.validators;
    for validator_it in validators {
        let validator = validator_it.clone().address;
        let delegation = utils::query_delegation(&deps, &env, &validator);

        let can_withdraw = delegation
            .map(|d| d.unwrap().accumulated_rewards[0].amount.u128())
            .unwrap_or(0);

        let withdraw_msg = DistributionMsg::WithdrawDelegatorReward { validator };

        msgs.push(CosmosMsg::Distribution(withdraw_msg));

        total_withdraw_amount += can_withdraw;
    }

    if total_withdraw_amount == 0 {
        return Err(
            ContractError::Std(
                StdError::generic_err("There is nothing to withdraw from validators")
            )
        );
    }

    let answer = to_json_binary(
        &(ExecuteResponse::WithdrawRewards {
            amount: Uint128::new(total_withdraw_amount),
            status: ResponseStatus::Success,
        })
    )?;

    Ok(Response::new().add_messages(msgs).set_data(answer))
}

pub fn try_redelegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_validator_address: String,
    old_validator_address: String,
    delegate_ratio: u128,
    recipient: Option<String>
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG_ITEM.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }

    // Validate new and old validator addresses
    // let validated_new_one = deps.api.addr_validate(&new_validator_address).unwrap();
    // assert_eq!(validated_new_one, new_validator_address);
    // let validated_old_one = deps.api.addr_validate(&old_validator_address).unwrap();
    // assert_eq!(validated_old_one, old_validator_address);

    // Check if the old_validator_address is in the contract's validators
    if config.validators.iter().all(|validator| validator.address != old_validator_address) {
        return Err(
            ContractError::Std(
                StdError::generic_err(
                    "The address of the validator you will replace does not exist in the state of the contract."
                )
            )
        );
    }

    if delegate_ratio > 100 || delegate_ratio == 0 {
        return Err(
            ContractError::Std(StdError::generic_err("Redelegate ratio have to be from 1 to 100!"))
        );
    }

    let delegation = utils::query_delegation(&deps, &env, &old_validator_address)?;

    if old_validator_address == new_validator_address {
        return Err(ContractError::Std(StdError::generic_err("Redelegation to the same validator")));
    }

    if delegation.is_none() {
        // Replace old_validator_address with new_validator_address
        if
            let Some(old_validator_position) = config.validators
                .iter()
                .position(|validator| validator.address == old_validator_address)
        {
            let old_validator = config.validators[old_validator_position].clone();
            let changed_weight = old_validator.weight
                .checked_mul(delegate_ratio)
                .unwrap()
                .checked_div(100)
                .unwrap();
            if
                let Some(new_validator_position) = config.validators
                    .iter()
                    .position(|validator| validator.address == new_validator_address)
            {
                // New validator address is in the list
                let new_validator = config.validators[new_validator_position].clone();
                config.validators[new_validator_position].weight = new_validator.weight
                    .checked_add(changed_weight)
                    .unwrap();
                config.validators[old_validator_position].weight = old_validator.weight
                    .checked_sub(changed_weight)
                    .unwrap();
            } else {
                // New validator address is not in the list
                let new_validator_weight = changed_weight;
                let old_validator_weight = old_validator.weight
                    .checked_sub(changed_weight)
                    .unwrap();
                config.validators[old_validator_position].weight = old_validator_weight;
                config.validators.push(ValidatorWithWeight {
                    address: new_validator_address.clone(),
                    weight: new_validator_weight,
                });
            }
            if delegate_ratio == 100 {
                config.validators.remove(old_validator_position);
            }
        }
        CONFIG_ITEM.save(deps.storage, &config)?;

        let answer = to_json_binary(
            &(ExecuteResponse::Redelegate {
                amount: Uint128::zero(),
                status: ResponseStatus::Success,
            })
        )?;

        return Ok(Response::new().set_data(answer));
    }

    let delegation = delegation.unwrap();
    let can_withdraw = delegation.accumulated_rewards[0].amount.u128();
    let can_redelegate = delegation.can_redelegate.amount.u128();
    let delegated_amount = delegation.amount.amount.u128();

    if
        can_redelegate <
        delegated_amount.checked_mul(delegate_ratio).unwrap().checked_div(100).unwrap()
    {
        return Err(
            ContractError::Std(StdError::generic_err("Cannot redelegate delegation amount"))
        );
    }

    // Replace old_validator_address with new_validator_address
    if
        let Some(old_validator_position) = config.validators
            .iter()
            .position(|validator| validator.address == old_validator_address)
    {
        let old_validator = config.validators[old_validator_position].clone();
        let changed_weight = old_validator.weight
            .checked_mul(delegate_ratio)
            .unwrap()
            .checked_div(100)
            .unwrap();
        if
            let Some(new_validator_position) = config.validators
                .iter()
                .position(|validator| validator.address == new_validator_address)
        {
            // New validator address is in the list
            let new_validator = config.validators[new_validator_position].clone();
            config.validators[new_validator_position].weight = new_validator.weight
                .checked_add(changed_weight)
                .unwrap();
            config.validators[old_validator_position].weight = old_validator.weight
                .checked_sub(changed_weight)
                .unwrap();
        } else {
            // New validator address is not in the list
            let new_validator_weight = changed_weight;
            let old_validator_weight = old_validator.weight.checked_sub(changed_weight).unwrap();
            config.validators[old_validator_position].weight = old_validator_weight;
            config.validators.push(ValidatorWithWeight {
                address: new_validator_address.clone(),
                weight: new_validator_weight,
            });
        }
        if delegate_ratio == 100 {
            config.validators.remove(old_validator_position);
        }
    }
    CONFIG_ITEM.save(deps.storage, &config)?;

    let mut messages = Vec::with_capacity(2);
    if can_withdraw != 0 {
        let admin = config.admin;
        let _recipient = recipient.unwrap_or(admin);
        let withdraw_msg: DistributionMsg = DistributionMsg::WithdrawDelegatorReward {
            validator: old_validator_address.clone(),
        };

        let msg = CosmosMsg::Distribution(withdraw_msg);

        messages.push(msg);
    }

    let redelegated_amount = can_redelegate
        .checked_mul(delegate_ratio)
        .unwrap()
        .checked_div(100)
        .unwrap();
    let coin = coin(redelegated_amount, ORAI);
    let redelegate_msg = StakingMsg::Redelegate {
        src_validator: old_validator_address,
        dst_validator: new_validator_address,
        amount: coin,
    };

    messages.push(CosmosMsg::Staking(redelegate_msg));
    let answer = to_json_binary(
        &(ExecuteResponse::Redelegate {
            amount: Uint128::new(can_redelegate),
            status: ResponseStatus::Success,
        })
    )?;

    return Ok(Response::new().add_messages(messages).set_data(answer));
}

fn query_config(deps: Deps) -> StdResult<QueryResponse> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    config.to_answer()
}

pub fn query_user_info(deps: Deps, address: String) -> StdResult<QueryResponse> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    let min_tier = config.min_tier();
    let user_info = USER_INFOS.may_load(deps.storage, address)?.unwrap_or(state::UserInfo {
        tier: min_tier,
        ..Default::default()
    });

    let answer = user_info.to_answer();
    return Ok(answer);
}

pub fn query_user_total_delegated(deps: Deps, address: String) -> StdResult<QueryResponse> {
    let user_total_delegated = USER_TOTAL_DELEGATED.may_load(
        deps.storage,
        address
    )?.unwrap_or_default();
    let answer = msg::QueryResponse::UserTotalDelegated {
        total_delegated: user_total_delegated,
    };
    return Ok(answer);
}

pub fn query_withdrawals(
    deps: Deps,
    address: String,
    start: Option<u32>,
    limit: Option<u32>
) -> StdResult<QueryResponse> {
    let withdrawals = WITHDRAWALS_LIST.may_load(deps.storage, address)?.unwrap_or_default();
    let amount = withdrawals.len();

    // The number of withdrawals can't exceed 50.
    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(50);

    let mut serialized_withdrawals: Vec<SerializedWithdrawals> = Vec::new();
    for i in start..start + limit {
        let index: usize = i.try_into().unwrap();
        if index < amount {
            serialized_withdrawals.push(withdrawals[index].to_serialized());
        }
    }

    let answer = QueryResponse::Withdrawals {
        amount: amount.try_into().unwrap(),
        withdrawals: serialized_withdrawals,
    };

    Ok(answer)
}

pub fn query_unbonds(deps: Deps) -> StdResult<QueryResponse> {
    let unbond_list = UNBOND_LIST;
    let amount = unbond_list.len(deps.storage)?;

    let mut serialized_unbonds: Vec<SerializedUnbonds> = Vec::new();

    let unbond_list_iter = unbond_list.iter(deps.storage)?;

    for it in unbond_list_iter {
        let user_unbond = it.unwrap();
        serialized_unbonds.push(user_unbond.to_serialized());
    }

    let answer = QueryResponse::Unbonds {
        amount: amount.try_into().unwrap(),
        unbonds: serialized_unbonds,
    };
    Ok(answer)
}
