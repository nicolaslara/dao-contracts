use cosmwasm_std::{Addr, DepsMut, Order};
use cw_auth_middleware::interface::Authorization;
use cw_auth_middleware::msg::{IsAuthorizedResponse, QueryMsg};
use cw_storage_plus::{Item, Map};

use crate::{msg::InstantiateMsg, ContractError};

pub struct AndContract {
    pub parent: Item<'static, Addr>,
    pub children: Map<'static, Addr, cosmwasm_std::Empty>,
}

impl AndContract {
    pub const fn new() -> Self {
        AndContract {
            parent: Item::new("dao"),
            children: Map::new("children"),
        }
    }

    pub fn instantiate(&self, deps: DepsMut, msg: InstantiateMsg) -> Result<(), ContractError> {
        self.parent.save(deps.storage, &msg.parent)?;
        for child in msg.children {
            self.children
                .save(deps.storage, child, &cosmwasm_std::Empty {})?;
        }
        Ok(())
    }
}

impl Authorization for AndContract {
    fn is_authorized(
        &self,
        storage: &dyn cosmwasm_std::Storage,
        querier: &cosmwasm_std::QuerierWrapper,
        msgs: &Vec<cosmwasm_std::CosmosMsg>,
        sender: &Addr,
    ) -> Result<bool, cw_auth_middleware::ContractError> {
        let children: Result<Vec<_>, _> = self
            .children
            .range(storage, None, None, Order::Ascending)
            .collect();
        if children.is_err() {
            return Ok(false);
        }
        let children = children.unwrap();

        // This checks all the registered authorizations return true
        Ok(children.into_iter().map(|c| c.0).all(|a| {
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

    fn get_sub_authorizations(
        &self,
        storage: &dyn cosmwasm_std::Storage,
    ) -> Result<Vec<Addr>, cw_auth_middleware::ContractError> {
        Ok(self
            .children
            .range(storage, None, None, Order::Ascending)
            .filter_map(|e| e.ok())
            .map(|e| e.0)
            .collect())
    }
}

pub const AND_CONTRACT: AndContract = AndContract::new();
