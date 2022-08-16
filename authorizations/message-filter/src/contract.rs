use cosmwasm_std::{entry_point, to_binary, Addr};
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{AuthorizationsResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Authorization, Config, MESSAGE_FILTER};
use crate::utils::str_to_value;
use cw_auth_middleware::ContractError as AuthorizationError;

const CONTRACT_NAME: &str = "crates.io:whitelist";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

use cw_auth_middleware::interface::Authorization as AuthorizationTrait;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config {
        dao: msg.dao,
        kind: msg.kind,
    };
    MESSAGE_FILTER.config.save(deps.storage, &config)?;
    Ok(Response::default().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddAuthorization { addr, msg } => {
            execute_add_authorization(deps, info, addr, msg)
        }

        ExecuteMsg::RemoveAuthorization { addr, msg } => {
            execute_remove_authorization(deps, info, addr.clone(), msg)
        }

        ExecuteMsg::UpdateExecutedAuthorizationState { msgs, sender } => MESSAGE_FILTER
            .update_authorization_state(deps.as_ref(), &msgs, &sender, &info.sender)
            .map_err(|e| ContractError::Unauthorized(e)),
    }
}

fn execute_add_authorization(
    deps: DepsMut,
    info: MessageInfo,
    authorized_addr: Addr,
    authorization_matcher: String,
) -> Result<Response, ContractError> {
    let config = MESSAGE_FILTER.config.load(deps.storage)?;
    if info.sender != config.dao {
        return Err(AuthorizationError::Unauthorized {
            reason: Some("Only the dao can add authorizations".to_string()),
        }
        .into());
    }

    // If the message can't be converted to a string, we fail
    str_to_value(&authorization_matcher)?;
    MESSAGE_FILTER.allowed.update(
        deps.storage,
        authorized_addr.clone(),
        |auth: Option<Vec<Authorization>>| -> Result<Vec<Authorization>, ContractError> {
            let new_auth = Authorization {
                addr: authorized_addr,
                matcher: authorization_matcher,
            };
            match auth {
                Some(mut auth) => {
                    auth.push(new_auth);
                    Ok(auth)
                }
                None => Ok(vec![new_auth]),
            }
        },
    )?;

    Ok(Response::default().add_attribute("action", "allow_message"))
}

fn execute_remove_authorization(
    deps: DepsMut,
    info: MessageInfo,
    authorized_addr: Addr,
    authorization_matcher: String,
) -> Result<Response, ContractError> {
    let config = MESSAGE_FILTER.config.load(deps.storage)?;
    if info.sender != config.dao {
        return Err(AuthorizationError::Unauthorized {
            reason: Some("Only the dao can add authorizations".to_string()),
        }
        .into());
    }

    MESSAGE_FILTER.allowed.update(
        deps.storage,
        authorized_addr.clone(),
        |auth: Option<Vec<Authorization>>| -> Result<Vec<Authorization>, ContractError> {
            match auth {
                Some(mut auth) => {
                    let i = auth
                        .iter()
                        .position(|x| *x.matcher == authorization_matcher);
                    if i.is_none() {
                        return Err(ContractError::NotFound {});
                    }
                    auth.remove(i.unwrap());
                    Ok(auth)
                }
                None => Err(ContractError::NotFound {}),
            }
        },
    )?;
    Ok(Response::default().add_attribute("action", "removed"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetAuthorizations { sender } => {
            let auths = MESSAGE_FILTER.allowed.may_load(deps.storage, sender)?;
            match auths {
                Some(authorizations) => to_binary(&AuthorizationsResponse { authorizations }),
                None => to_binary(&AuthorizationsResponse {
                    authorizations: vec![],
                }),
            }
        }
        QueryMsg::Authorize { msgs, sender } => {
            MESSAGE_FILTER.query_authorizations(deps, msgs, sender)
        }
    }
}
