use cosmwasm_std::{
    to_binary, wasm_execute, Addr, Binary, CosmosMsg, CustomMsg, Deps, DepsMut, Empty, Env,
    MessageInfo, QuerierWrapper, Reply, Response, StdResult, Storage, SubMsg,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::IsAuthorizedResponse;
use crate::ContractError;

const UPDATE_REPLY_ID: u64 = 1000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthoriazationExecuteMsg<ExecuteExt = Empty>
where
    ExecuteExt: CustomMsg,
{
    /// Some authorizations may want to track information about the users or
    /// messages to determine if they authorize or not. This message should be
    /// sent every time the authorizations are successfully used so that
    /// sub-authorizations can update their internal state.
    UpdateExecutedAuthorizationState {
        msgs: Vec<CosmosMsg>,
        sender: Addr,
    },

    // Extensions allow implementors to add their own custom messages to the contract
    Extension(ExecuteExt),
}

pub trait Authorization<ExecuteExt = Empty>
where
    ExecuteExt: CustomMsg,
{
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

    fn execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: AuthoriazationExecuteMsg<ExecuteExt>,
    ) -> Result<Response, ContractError> {
        match msg {
            AuthoriazationExecuteMsg::UpdateExecutedAuthorizationState { msgs, sender } => {
                self.update_authorization_state(deps.as_ref(), &msgs, &sender, &info.sender)
            }
            AuthoriazationExecuteMsg::Extension(msg) => {
                self.handle_execute_extension(deps, env, info, msg)
            }
        }
    }

    fn handle_execute_extension(
        &self,
        _deps: DepsMut,
        _env: Env,
        _info: MessageInfo,
        _msg: ExecuteExt,
    ) -> Result<Response, ContractError> {
        Ok(Response::default())
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
                        &AuthoriazationExecuteMsg::<ExecuteExt>::UpdateExecutedAuthorizationState {
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
