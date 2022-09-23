use cosmwasm_std::{Addr, DepsMut};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ContractError;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct Config {
    /// The address of the DAO that this authorization module is
    /// associated with.
    pub dao: Addr,
}

pub struct AuthorizationMiddlewareState {
    pub config: Item<'static, Config>,
    pub authorization: Item<'static, Addr>,
}

impl AuthorizationMiddlewareState {
    pub const fn new() -> Self {
        AuthorizationMiddlewareState {
            config: Item::new("config"),
            authorization: Item::new("authorization"),
        }
    }

    pub fn instantiate(&self, deps: DepsMut, sender: Addr) -> Result<(), ContractError> {
        self.config.save(deps.storage, &Config { dao: sender })?;
        Ok(())
    }
}
