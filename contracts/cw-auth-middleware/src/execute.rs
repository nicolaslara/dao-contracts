use cosmwasm_std::{wasm_execute, Addr, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response};

use crate::{interface::Authorization, state::AUTH_MANAGER, ContractError};

// This method allows this contract to behave as a proposal. For this to work, the contract needs to have been instantiated by a dao.
pub fn execute(deps: Deps, msgs: Vec<CosmosMsg>, sender: Addr) -> Result<Response, ContractError> {
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

pub fn add_authorization(
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

pub fn remove_authorization(
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
