#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    wasm_execute, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdResult,
};
use cw2::set_contract_version;

use crate::interface::Authorization;

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Config, AUTH_MANAGER},
};

const CONTRACT_NAME: &str = "crates.io:cw-auth-manager";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const UPDATE_REPLY_ID: u64 = 1000;

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
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddAuthorization { auth_contract } => {
            execute_add_authorization(deps, env, info, auth_contract)
        }
        ExecuteMsg::RemoveAuthorization { auth_contract } => {
            execute_remove_authorization(deps, info, auth_contract.to_string())
        }
        ExecuteMsg::UpdateExecutedAuthorizationState { msgs, sender } => {
            AUTH_MANAGER.update_authorization_state(deps.as_ref(), &msgs, &sender, &info.sender)
        }
        ExecuteMsg::Execute { msgs } => execute_execute(deps.as_ref(), msgs, info.sender),
        ExecuteMsg::ReplaceOwner { new_dao } => {
            let mut config = AUTH_MANAGER.config.load(deps.storage)?;
            if info.sender != config.dao {
                Err(ContractError::Unauthorized { reason: None })
            } else {
                config.dao = new_dao.clone();
                AUTH_MANAGER.config.save(deps.storage, &config)?;
                Ok(Response::default()
                    .add_attribute("action", "replace_dao")
                    .add_attribute("new_dao", new_dao))
            }
        }
    }
}

// This method allows this contract to behave as a proposal. For this to work, the contract needs to have been instantiated by a dao.
fn execute_execute(
    deps: Deps,
    msgs: Vec<CosmosMsg>,
    sender: Addr,
) -> Result<Response, ContractError> {
    if msgs.is_empty() {
        return Err(ContractError::InvalidProposal {});
    }
    let config = AUTH_MANAGER.config.load(deps.storage)?;

    let response = AUTH_MANAGER.update_authorization_state(deps, &msgs, &sender, &sender)?;
    let execute_msg = wasm_execute(
        config.dao.to_string(),
        &cw_core::msg::ExecuteMsg::ExecuteProposalHook { msgs },
        vec![],
    )?;

    Ok(response.add_message(execute_msg))
}

pub fn execute_add_authorization(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = AUTH_MANAGER.config.load(deps.storage)?;
    if config.dao != info.sender {
        // Only DAO can add authorizations
        return Err(ContractError::Unauthorized {
            reason: Some("Sender can't add authorization.".to_string()),
        });
    }

    // ToDo: Verify that this is an auth?
    let validated_address = deps.api.addr_validate(&address)?;
    AUTH_MANAGER.authorizations.update(
        deps.storage,
        config.dao,
        |auths| -> Result<Vec<Addr>, ContractError> {
            match auths {
                Some(mut l) => {
                    l.push(validated_address);
                    Ok(l)
                }
                None => Ok(vec![validated_address]),
            }
        },
    )?;

    Ok(Response::default()
        .add_attribute("action", "add_authorizations")
        .add_attribute("address", address))
}

pub fn execute_remove_authorization(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = AUTH_MANAGER.config.load(deps.storage)?;
    if config.dao != info.sender {
        // Only DAO can remove authorizations
        return Err(ContractError::Unauthorized {
            reason: Some("Sender can't remove authorization.".to_string()),
        });
    }

    let validated_address = deps.api.addr_validate(&address)?;
    AUTH_MANAGER
        .authorizations
        .remove(deps.storage, validated_address);

    Ok(Response::default()
        .add_attribute("action", "remove_authorization")
        .add_attribute("address", address))
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
        UPDATE_REPLY_ID => {
            if msg.result.is_err() {
                return Ok(Response::new().add_attribute("update_error", msg.result.unwrap_err()));
            }
            Ok(Response::new()
                .add_attribute("update_success", format!("{:?}", msg.result.unwrap())))
        }
        id => Err(ContractError::Std(cosmwasm_std::StdError::GenericErr {
            msg: format!("Unknown reply id: {}", id),
        })),
    }
}
