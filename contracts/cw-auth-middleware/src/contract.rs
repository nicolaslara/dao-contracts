#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdResult};
use cw2::set_contract_version;

use crate::interface::{AuthoriazationExecuteMsg, Authorization};

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Config, AUTH_MANAGER},
};

const CONTRACT_NAME: &str = "crates.io:cw-auth-manager";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config {
        dao: info.sender.clone(),
    };
    AUTH_MANAGER.instantiate(deps.storage, &config, info.sender)?;

    Ok(Response::default().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: AuthoriazationExecuteMsg<ExecuteMsg>,
) -> Result<Response, ContractError> {
    AUTH_MANAGER.execute(deps, env, info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Authorize { msgs, sender } => {
            AUTH_MANAGER.query_authorizations(deps, msgs, sender)
        }
        QueryMsg::GetAuthorizations { .. } => {
            unimplemented!()
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        // Update reply errors are always ignored.
        id if id == AUTH_MANAGER.get_update_reply_id() => AUTH_MANAGER.sub_message_reply(msg),
        id => Err(ContractError::Std(cosmwasm_std::StdError::GenericErr {
            msg: format!("Unknown reply id: {}", id),
        })),
    }
}
