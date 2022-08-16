use cosmwasm_std::{Addr, Response};
use cw_auth_middleware::interface::Authorization as AuthorizationTrait;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    utils::{deep_partial_match, msg_to_value, str_to_value},
    ContractError,
};
use cw_auth_middleware::ContractError as AuthorizationError;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum Kind {
    Allow {},
    Reject {},
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct Config {
    /// The address of the DAO that this authorization module is
    /// associated with.
    pub dao: Addr,
    /// The type of authorization this is. Kind::Allow means messages will only
    /// be authorized (allowed) if there is a matching Authorization in the
    /// contract. Kind::Reject means all messages will be authorized (allowed)
    /// by this contract unless explicitly rejected by one of the stored
    /// authorizations
    pub kind: Kind,
}

impl Config {
    pub fn default_response(&self) -> Result<Response, ContractError> {
        match self.kind {
            Kind::Allow {} => Err(AuthorizationError::Unauthorized {
                reason: Some("No authorizations allowed the request. Rejecting.".to_string()),
            }
            .into()),
            Kind::Reject {} => Ok(Response::default()
                .add_attribute("allowed", "true")
                .add_attribute(
                    "reason",
                    "No authorizations rejected the request. Allowing.",
                )),
        }
    }

    pub fn default_authorization(&self) -> bool {
        match self.kind {
            Kind::Allow {} => false,
            Kind::Reject {} => true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Authorization {
    pub addr: Addr,
    /// A json representation of a CosmosMsg. Incomming messages will be
    /// recursively compared to the matcher to determine if they are authorized.
    ///
    /// To short-circuit the recursive comparison (i.e.: allow everything under
    /// an object key), you can use the empty object.
    ///
    /// For example:
    ///
    /// {"bank": {"to_address": "an_address", "amount":[{"denom": "juno", "amount": 1}]}}
    ///
    /// will match exactly that message but not a message where any of the fields are different.
    ///
    /// However, {"bank": {}} will match all bank messages, and
    /// {"bank": {"send": {"to_address": "an_address", "amount": {}}}} will match all bank messages to "an_address".
    ///
    pub matcher: String,
}

pub struct MessageFilter {
    pub config: Item<'static, Config>,
    pub allowed: Map<'static, Addr, Vec<Authorization>>,
}

impl MessageFilter {
    pub const fn new() -> Self {
        MessageFilter {
            config: Item::new("config"),
            allowed: Map::new("allowed"),
        }
    }
}

impl AuthorizationTrait for MessageFilter {
    fn is_authorized(
        &self,
        storage: &dyn cosmwasm_std::Storage,
        _querier: &cosmwasm_std::QuerierWrapper,
        msgs: &Vec<cosmwasm_std::CosmosMsg>,
        sender: &Addr,
    ) -> Result<bool, cw_auth_middleware::ContractError> {
        let config = self.config.load(storage)?;
        let auths = self.allowed.load(storage, sender.clone());

        // If there are no auths, return the default for each Kind
        if auths.is_err() {
            return Ok(config.default_authorization());
        }

        let auths = auths.unwrap();

        // check that all messages can be converted to values
        for m in msgs {
            msg_to_value(&m).map_err(|e| cw_auth_middleware::ContractError::Unauthorized {
                reason: Some(e.to_string()),
            })?;
        }
        // check that all auths can be converted to values
        for a in &auths {
            str_to_value(&a.matcher).map_err(|e| {
                cw_auth_middleware::ContractError::Unauthorized {
                    reason: Some(e.to_string()),
                }
            })?;
        }

        let matched = auths.iter().any(|a| {
            msgs.iter().all(|m| {
                deep_partial_match(
                    &msg_to_value(&m).unwrap(),
                    &str_to_value(&a.matcher).unwrap(),
                )
            })
        });

        if matched {
            return match config.kind {
                Kind::Allow {} => Ok(true),
                Kind::Reject {} => Ok(false),
            };
        }
        Ok(config.default_authorization())
    }

    fn get_sub_authorizations(
        &self,
        _storage: &dyn cosmwasm_std::Storage,
    ) -> Result<Vec<Addr>, cw_auth_middleware::ContractError> {
        Ok(vec![])
    }
}

pub const MESSAGE_FILTER: MessageFilter = MessageFilter::new();
