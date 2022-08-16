use cosmwasm_std::{
    to_binary, wasm_execute, Addr, Binary, CosmosMsg, Deps, QuerierWrapper, Reply, Response,
    StdResult, Storage, SubMsg,
};

use crate::{
    msg::{ExecuteMsg, IsAuthorizedResponse},
    ContractError,
};

const UPDATE_REPLY_ID: u64 = 1000;

pub trait Authorization {
    fn is_authorized(
        &self,
        storage: &dyn Storage,
        querier: &QuerierWrapper,
        msgs: &Vec<CosmosMsg>,
        sender: &Addr,
    ) -> Result<bool, ContractError>;

    fn query_authorizations(
        &self,
        deps: Deps,
        msgs: Vec<CosmosMsg>,
        sender: Addr,
    ) -> StdResult<Binary> {
        to_binary(&IsAuthorizedResponse {
            authorized: self
                .is_authorized(deps.storage, &deps.querier, &msgs, &sender)
                .unwrap_or(false),
        })
    }

    fn get_sub_authorizations(&self, storage: &dyn Storage) -> Result<Vec<Addr>, ContractError>;
    fn get_update_reply_id(&self) -> u64 {
        UPDATE_REPLY_ID
    }

    fn update_sub_authorization_msgs(
        &self,
        storage: &dyn Storage,
        msgs: &Vec<CosmosMsg>,
        sender: &Addr,
    ) -> Result<Vec<SubMsg>, ContractError> {
        let auths = self.get_sub_authorizations(storage)?;
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
                    self.get_update_reply_id(),
                ))
            })
            .collect();

        Ok(msgs?)
    }

    fn update_authorization_state(
        &self,
        deps: Deps,
        msgs: &Vec<CosmosMsg>,
        sender: &Addr,
        _real_sender: &Addr,
    ) -> Result<Response, ContractError> {
        let sub_msgs = self.update_sub_authorization_msgs(deps.storage, msgs, sender)?;
        Ok(Response::default().add_submessages(sub_msgs))
    }

    fn sub_message_reply(&self, msg: Reply) -> Result<Response, ContractError> {
        if msg.result.is_err() {
            return Ok(Response::new().add_attribute("update_error", msg.result.unwrap_err()));
        }
        Ok(Response::new().add_attribute("update_success", format!("{:?}", msg.result.unwrap())))
    }
}
