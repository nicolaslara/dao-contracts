#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    wasm_execute, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdResult,
};
use cw2::set_contract_version;

use cw_authorizations::{
    msg::{AuthoriazationExecuteMsg, AuthoriazationQueryMsg},
    Authorization, AuthorizationError,
};

use crate::{
    msg::{ExecuteMsg, InstantiateMsg, IsAuthorizedResponse},
    state::AuthorizationMiddlewareState,
    ContractError,
};

const CONTRACT_NAME: &str = "crates.io:cw-auth-manager";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AuthorizationMiddlewareContract {
    state: AuthorizationMiddlewareState,
}

impl Authorization<ExecuteMsg, Empty, ContractError> for AuthorizationMiddlewareContract {
    fn new() -> Self {
        AuthorizationMiddlewareContract {
            state: AuthorizationMiddlewareState::new(),
        }
    }

    fn is_authorized(
        &self,
        deps: Deps,
        msgs: &Vec<CosmosMsg>,
        sender: &Addr,
    ) -> Result<bool, AuthorizationError<ContractError>> {
        let auth_contract = self.state.authorization.may_load(deps.storage)?;
        Ok(match auth_contract {
            None => false,
            Some(auth_contract) => {
                let response: IsAuthorizedResponse = deps.querier.query_wasm_smart(
                    auth_contract,
                    &AuthoriazationQueryMsg::IsAuthorized::<Empty> {
                        msgs: msgs.to_owned(),
                        sender: sender.to_owned(),
                    },
                )?;
                response.authorized
            }
        })
    }

    fn get_sub_authorizations(
        &self,
        _deps: Deps,
    ) -> Result<Vec<Addr>, AuthorizationError<ContractError>> {
        Ok(vec![])
    }

    fn execute_extension(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response, AuthorizationError<ContractError>> {
        let config = self.state.config.load(deps.storage)?;
        if info.sender != config.dao {
            return Err(AuthorizationError::ContractError(
                ContractError::UnauthorizedBecause {
                    reason: "Only the dao can add or remove authorizations".to_string(),
                },
            ));
        }

        match msg {
            ExecuteMsg::SetAuthorization { auth_contract } => {
                self.set_authorization(deps, info, auth_contract)
            }
            ExecuteMsg::Execute { msgs } => self.execute_proposal(deps.as_ref(), msgs, info.sender),
        }
        .map_err(|e| AuthorizationError::ContractError(e))
    }
}

impl AuthorizationMiddlewareContract {
    pub fn set_authorization(
        self: &Self,
        deps: DepsMut,
        info: MessageInfo,
        address: Addr,
    ) -> Result<Response, ContractError> {
        let config = self.state.config.load(deps.storage)?;
        if config.dao != info.sender {
            // Only DAO can add authorizations
            return Err(ContractError::UnauthorizedBecause {
                reason: "Sender can't set authorization.".to_string(),
            });
        }

        self.state.authorization.save(deps.storage, &address)?;

        Ok(Response::default()
            .add_attribute("action", "add_authorizations")
            .add_attribute("address", address))
    }

    // This method allows this contract to behave as a proposal. For this to work, the contract needs to have been instantiated by a dao.
    pub fn execute_proposal(
        self: &Self,
        deps: Deps,
        msgs: Vec<CosmosMsg>,
        sender: Addr,
    ) -> Result<Response, ContractError> {
        if msgs.is_empty() {
            return Err(ContractError::InvalidProposal {});
        }
        let config = self.state.config.load(deps.storage)?;

        let response = self
            .update_authorization_state(deps, &msgs, &sender, &sender)
            .map_err(|e| ContractError::UnauthorizedBecause {
                reason: e.to_string(),
            })?;

        let execute_msg = wasm_execute(
            config.dao.to_string(),
            &cw_core::msg::ExecuteMsg::ExecuteProposalHook { msgs },
            vec![],
        )?;

        Ok(response.add_message(execute_msg))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, AuthorizationError<ContractError>> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    AuthorizationMiddlewareContract::new()
        .state
        .instantiate(deps, info.sender)
        .map_err(|e| AuthorizationError::ContractError(e))?;
    Ok(Response::default().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: AuthoriazationExecuteMsg<ExecuteMsg>,
) -> Result<Response, AuthorizationError<ContractError>> {
    AuthorizationMiddlewareContract::new().execute(deps, env, info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: AuthoriazationQueryMsg) -> StdResult<Binary> {
    AuthorizationMiddlewareContract::new().query(deps, env, msg)
}
