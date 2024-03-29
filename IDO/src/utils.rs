use crate::{ msg::ContractStatus, state::{ Config, Ido, CONFIG_KEY } };
use cosmwasm_std::{ Coin, StdError, StdResult, Storage, DepsMut };

pub fn assert_contract_active(storage: &dyn Storage) -> StdResult<()> {
    let config = Config::load(storage)?;
    let active_status = ContractStatus::Active as u8;

    if config.status != active_status {
        return Err(StdError::generic_err("Contract is not active"));
    }

    Ok(())
}

pub fn assert_admin(deps: &DepsMut, address: &String) -> StdResult<()> {
    let canonical_admin = address.clone();
    let config = CONFIG_KEY.load(deps.storage)?;

    if config.admin != canonical_admin {
        return Err(StdError::generic_err("Unauthorized"));
    }

    Ok(())
}

pub fn assert_ido_admin(deps: &DepsMut, address: &String, ido_id: u32) -> StdResult<()> {
    let canonical_admin = address.clone();
    let ido = Ido::load(deps.storage, ido_id)?;

    if ido.admin != canonical_admin {
        return Err(StdError::generic_err("Unauthorized"));
    }

    Ok(())
}

pub fn sent_funds(coins: &[Coin]) -> StdResult<u128> {
    let mut amount: u128 = 0;

    for coin in coins {
        if coin.denom != "orai" {
            return Err(StdError::generic_err("Unsopported token"));
        }

        amount = amount.checked_add(coin.amount.u128()).unwrap();
    }

    Ok(amount)
}
