use cosmwasm_std::{Addr, CosmosMsg, Deps, QuerierWrapper, Response, Storage};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    execute,
    interface::Authorization,
    msg::{ExecuteMsg, IsAuthorizedResponse, QueryMsg},
    ContractError,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct Config {
    /// The address of the DAO that this authorization module is
    /// associated with.
    pub dao: Addr,
}

pub struct AuthorizationManager {
    pub config: Item<'static, Config>,
    pub authorizations: Map<'static, Addr, Vec<Addr>>,
}

impl AuthorizationManager {
    pub const fn new() -> Self {
        AuthorizationManager {
            config: Item::new("config"),
            authorizations: Map::new("authorizations"),
        }
    }

    pub fn instantiate(
        &self,
        storage: &mut dyn Storage,
        config: &Config,
        sender: Addr,
    ) -> Result<(), ContractError> {
        self.config.save(storage, config)?;
        let empty: Vec<Addr> = vec![];
        self.authorizations.save(storage, sender, &empty)?;
        Ok(())
    }
}

impl Authorization<ExecuteMsg> for AuthorizationManager {
    fn is_authorized(
        &self,
        storage: &dyn Storage,
        querier: &QuerierWrapper,
        msgs: &Vec<CosmosMsg>,
        sender: &Addr,
    ) -> Result<bool, crate::ContractError> {
        // Right now this defaults to an *or*. We could update the contract to
        // support a custom allow/reject behaviour (similarly to how it's done in
        // message-filter)
        let config = self.config.load(storage)?;
        let auths = self.authorizations.load(storage, config.dao)?;

        if auths.is_empty() {
            return Ok(false);
        }

        Ok(auths.into_iter().any(|a| {
            querier
                .query_wasm_smart(
                    a.clone(),
                    &QueryMsg::Authorize {
                        msgs: msgs.clone(),
                        sender: sender.clone(),
                    },
                )
                .unwrap_or(IsAuthorizedResponse { authorized: false })
                .authorized
        }))
    }

    fn get_sub_authorizations(&self, storage: &dyn Storage) -> Result<Vec<Addr>, ContractError> {
        let config = self.config.load(storage)?;
        Ok(self.authorizations.load(storage, config.dao)?)
    }

    fn update_authorization_state(
        &self,
        deps: Deps,
        msgs: &Vec<CosmosMsg>,
        sender: &Addr,
        real_sender: &Addr,
    ) -> Result<Response, ContractError> {
        let config = self.config.load(deps.storage)?;
        if sender != real_sender && real_sender != &config.dao {
            return Err(ContractError::Unauthorized {
            reason: Some("Auth updates that aren't triggered by a parent contract cannot specify a sender other than the caller ".to_string()),
        });
        }

        // If at least one authorization module authorized this message, we send the
        // Authorize execute message to all the authorizations so that they can update their
        // state if needed.
        if self.is_authorized(deps.storage, &deps.querier, &msgs, &sender)? {
            let sub_msgs = self.update_sub_authorization_msgs(deps.storage, msgs, sender)?;
            Ok(Response::default().add_submessages(sub_msgs))
        } else {
            Err(ContractError::Unauthorized {
                reason: Some("No sub authorization passed".to_string()),
            })
        }
    }

    fn handle_execute_extension(
        &self,
        deps: cosmwasm_std::DepsMut,
        env: cosmwasm_std::Env,
        info: cosmwasm_std::MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response, ContractError> {
        match msg {
            ExecuteMsg::AddAuthorization { auth_contract } => {
                execute::add_authorization(deps, env, info, auth_contract)
            }
            ExecuteMsg::RemoveAuthorization { auth_contract } => {
                execute::remove_authorization(deps, info, auth_contract.to_string())
            }
            ExecuteMsg::UpdateExecutedAuthorizationState { msgs, sender } => {
                AUTH_MANAGER.update_authorization_state(deps.as_ref(), &msgs, &sender, &info.sender)
            }
            ExecuteMsg::Execute { msgs } => execute::execute(deps.as_ref(), msgs, info.sender),
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
}

pub const AUTH_MANAGER: AuthorizationManager = AuthorizationManager::new();
