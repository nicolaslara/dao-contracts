#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, wasm_execute, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Reply,
    Response, StdResult, SubMsg,
};
use cw2::set_contract_version;

use crate::msg::IsAuthorizedResponse;
use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Config, AUTHORIZATIONS, CONFIG},
    utils::check_authorization,
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
    let empty: Vec<Addr> = vec![];
    CONFIG.save(deps.storage, &config)?;
    AUTHORIZATIONS.save(deps.storage, &info.sender, &empty)?;

    Ok(Response::default().add_attribute("action", "instantiate"))
}

fn authorize_messages(
    deps: Deps,
    msgs: Vec<CosmosMsg>,
    sender: Addr,
) -> Result<bool, ContractError> {
    // This checks all the registered authorizations
    let config = CONFIG.load(deps.storage)?;
    let auths = AUTHORIZATIONS.load(deps.storage, &config.dao)?;

    if auths.is_empty() {
        return Ok(false);
    }

    Ok(check_authorization(deps, &auths, &msgs, &sender))
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
            execute_update_authorization_state(deps.as_ref(), msgs, sender, info.sender)
        }
        ExecuteMsg::Execute { msgs } => execute_execute(deps.as_ref(), msgs, info.sender),
        ExecuteMsg::ReplaceOwner { new_dao } => {
            let mut config = CONFIG.load(deps.storage)?;
            if info.sender != config.dao {
                Err(ContractError::Unauthorized { reason: None })
            } else {
                config.dao = new_dao.clone();
                CONFIG.save(deps.storage, &config)?;
                Ok(Response::default()
                    .add_attribute("action", "replace_dao")
                    .add_attribute("new_dao", new_dao))
            }
        }
    }
}

fn execute_update_authorization_state(
    deps: Deps,
    msgs: Vec<CosmosMsg>,
    sender: Addr,
    real_sender: Addr,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if sender != real_sender && real_sender != config.dao {
        return Err(ContractError::Unauthorized {
            reason: Some("Auth updates that aren't triggered by a parent contract cannot specify a sender other than the caller ".to_string()),
        });
    }

    // If at least one authorization module authorized this message, we send the
    // Authorize execute message to all the authorizations so that they can update their
    // state if needed.
    if authorize_messages(deps, msgs.clone(), sender.clone())? {
        let auths = AUTHORIZATIONS.load(deps.storage, &config.dao)?;

        let msgs: Result<Vec<_>, _> = auths
            .iter()
            .map(|auth| -> Result<SubMsg, ContractError> {
                // All errors from submessages are ignored since the validation has already been done above.
                // In the future we may need a better way to handle updates
                Ok(SubMsg::reply_on_error(
                    wasm_execute(
                        auth.to_string(),
                        &ExecuteMsg::UpdateExecutedAuthorizationState {
                            msgs: msgs.clone(),
                            sender: sender.clone(),
                        },
                        vec![],
                    )?,
                    UPDATE_REPLY_ID,
                ))
            })
            .collect();

        Ok(Response::default()
            .add_attribute("action", "execute_authorize")
            .add_attribute("authorized", "true")
            .add_submessages(msgs?))
    } else {
        Err(ContractError::Unauthorized { reason: None })
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
    let config = CONFIG.load(deps.storage)?;

    let response = execute_update_authorization_state(
        deps.clone(),
        msgs.clone(),
        sender.clone(),
        sender.clone(),
    )?;
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
    let config = CONFIG.load(deps.storage)?;
    if config.dao != info.sender {
        // Only DAO can add authorizations
        return Err(ContractError::Unauthorized {
            reason: Some("Sender can't add authorization.".to_string()),
        });
    }

    // ToDo: Verify that this is an auth?
    let validated_address = deps.api.addr_validate(&address)?;
    AUTHORIZATIONS.update(
        deps.storage,
        &config.dao,
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
    let config = CONFIG.load(deps.storage)?;
    if config.dao != info.sender {
        // Only DAO can remove authorizations
        return Err(ContractError::Unauthorized {
            reason: Some("Sender can't remove authorization.".to_string()),
        });
    }

    let validated_address = deps.api.addr_validate(&address)?;
    AUTHORIZATIONS.remove(deps.storage, &validated_address);

    Ok(Response::default()
        .add_attribute("action", "remove_authorization")
        .add_attribute("address", address))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Authorize { msgs, sender } => query_authorizations(deps, msgs, sender),
        QueryMsg::GetAuthorizations { .. } => {
            unimplemented!()
        }
    }
}

fn query_authorizations(deps: Deps, msgs: Vec<CosmosMsg>, sender: Addr) -> StdResult<Binary> {
    to_binary(&IsAuthorizedResponse {
        authorized: authorize_messages(deps, msgs, sender).unwrap_or(false),
    })
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
