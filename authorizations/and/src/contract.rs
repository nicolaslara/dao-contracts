#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
use cw_auth_middleware::msg::QueryMsg;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::AND_CONTRACT;
use cw_auth_middleware::ContractError as AuthorizationError;

use cw_auth_middleware::interface::Authorization;

const CONTRACT_NAME: &str = "crates.io:whitelist";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    AND_CONTRACT.instantiate(deps, msg)?;
    Ok(Response::default().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, AuthorizationError> {
    if info.sender != AND_CONTRACT.parent.load(deps.storage)? {
        return Err(AuthorizationError::Unauthorized {
            reason: Some("Only the parent can execute on this contract".to_string()),
        }
        .into());
    }

    match msg {
        ExecuteMsg::AddChild { addr } => {
            AND_CONTRACT.children.save(deps.storage, addr, &Empty {})?;
            Ok(Response::default().add_attribute("action", "allow"))
        }
        ExecuteMsg::RemoveChild { addr } => {
            AND_CONTRACT.children.remove(deps.storage, addr);
            Ok(Response::default().add_attribute("action", "remove"))
        }
        ExecuteMsg::UpdateExecutedAuthorizationState { msgs, sender } => {
            AND_CONTRACT.update_authorization_state(deps.as_ref(), &msgs, &sender, &info.sender)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Authorize { msgs, sender } => {
            AND_CONTRACT.query_authorizations(deps, msgs, sender)
        }
        _ => unimplemented!(),
    }
}

#[cfg(test)]
mod tests {}
