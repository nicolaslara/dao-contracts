use cosmwasm_std::{CosmosMsg, Uint128};
use schemars::{JsonSchema, Map};
use serde::{Deserialize, Serialize};

use serde_json_wasm::{from_str, to_string};

use crate::ContractError;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    U128(Uint128),
    Number(u64), // Why doesn't u128 work here?
    String(String),
    Array(Vec<Value>),
    Object(Map<String, Value>),
}

pub fn msg_to_value(msg: &CosmosMsg) -> Result<Value, ContractError> {
    let serialized = to_string(msg).map_err(|_| ContractError::CustomError {
        val: "invalid CosmosMsg".to_string(),
    })?;

    str_to_value(&serialized)
}

pub fn str_to_value(msg: &str) -> Result<Value, ContractError> {
    from_str(msg).map_err(|_| ContractError::CustomError {
        val: "invalid str".to_string(),
    })
}

pub fn deep_partial_match(msg: &Value, authorization: &Value) -> bool {
    match authorization {
        Value::Object(auth_map) => {
            if auth_map.is_empty() {
                return true;
            }

            let mut matching = true;
            if let Value::Object(msg_map) = msg {
                for (key, val) in auth_map {
                    if !msg_map.contains_key(key) {
                        return false;
                    };
                    matching = matching & deep_partial_match(msg_map.get(key).unwrap(), val);
                }
            } else {
                return false;
            }
            matching
        }
        Value::Array(auth_array) => {
            // Comparing arrays manually because PartialEq doesn't understand use our deep matching.
            let mut matching = true;
            if let Value::Array(msg_array) = msg {
                if msg_array.len() != auth_array.len() {
                    return false;
                }
                for (i, elem) in auth_array.iter().enumerate() {
                    matching = matching & deep_partial_match(&msg_array[i], &elem);
                }
            } else {
                return false;
            }
            matching
        }
        _ => authorization == msg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::{coins, BankMsg, CosmosMsg};
    use serde_json_wasm::from_str;

    #[test]
    fn test_deep_partial_match_simple() {
        let to_address = String::from("you");
        let amount = coins(1015, "earth");
        let bank = BankMsg::Send { to_address, amount };
        let msg: CosmosMsg = bank.clone().into();

        // Comparing a cosmos message to partial json
        assert_eq!(
            deep_partial_match(
                &msg_to_value(&msg).unwrap(),
                &from_str(r#"{"bank": {}}"#).unwrap()
            ),
            true,
        );

        // Non-matching messages should fail
        assert_eq!(
            deep_partial_match(
                &from_str(r#"{"test": 1}"#).unwrap(),
                &from_str(r#"{"bank": {}}"#).unwrap()
            ),
            false,
        );

        // Partial messages work
        assert_eq!(
            deep_partial_match(
                &from_str(r#"{"bank": [1,2,3]}"#).unwrap(),
                &from_str(r#"{"bank": {}}"#).unwrap()
            ),
            true
        );

        // Testing array comparison as a proxy for all other Eq for Values
        assert_eq!(
            deep_partial_match(
                &from_str(r#"{"bank": [1,3,2]}"#).unwrap(),
                &from_str(r#"{"bank": [1,2,3]}"#).unwrap(),
            ),
            false
        );
        assert_eq!(
            deep_partial_match(
                &from_str(r#"{"bank": [1,2,3]}"#).unwrap(),
                &from_str(r#"{"bank": [1,2,3]}"#).unwrap(),
            ),
            true
        );

        // The partial json comparison only works in one direction
        assert_eq!(
            deep_partial_match(
                &from_str(r#"{"bank": {}}"#).unwrap(),
                &from_str(r#"{"bank": [1,2,3]}"#).unwrap()
            ),
            false
        );

        // The partial json comparison works with any json type
        assert_eq!(
            deep_partial_match(
                &from_str(r#"{"send": {"to_address": {}}}"#).unwrap(),
                &from_str(r#"{"send": {"to_address": "test"}}"#).unwrap()
            ),
            false
        );

        assert_eq!(
            deep_partial_match(
                &from_str(r#"{"send": {"to_address": "test"}}"#).unwrap(),
                &from_str(r#"{"send": {"to_address": {}}}"#).unwrap(),
            ),
            true
        );
    }

    #[test]
    fn test_deep_partial_match_complex() {
        let to_address = String::from("an_address");
        let amount = coins(1015, "earth");
        let bank = BankMsg::Send {
            to_address: to_address.clone(),
            amount,
        };
        let msg: CosmosMsg = bank.clone().into();

        assert_eq!(
            deep_partial_match(
                &msg_to_value(&msg).unwrap(),
                &from_str(r#"{"bank": {"send": {"to_address": "an_address", "amount": {}}}}"#)
                    .unwrap(),
            ),
            true
        );

        // Changing amouont
        let amount = coins(1234, "juno");
        let bank = BankMsg::Send {
            to_address: to_address.clone(),
            amount,
        };
        let msg: CosmosMsg = bank.clone().into();

        assert_eq!(
            deep_partial_match(
                &msg_to_value(&msg).unwrap(),
                &from_str(r#"{"bank": {"send": {"to_address": "an_address", "amount": {}}}}"#)
                    .unwrap(),
            ),
            true
        );

        // Changing address
        let amount = coins(1234, "juno");
        let bank = BankMsg::Send {
            to_address: "other_addr".to_string(),
            amount,
        };
        let msg: CosmosMsg = bank.clone().into();

        assert_eq!(
            deep_partial_match(
                &msg_to_value(&msg).unwrap(),
                &from_str(r#"{"bank": {"send": {"to_address": "an_address", "amount": {}}}}"#)
                    .unwrap(),
            ),
            false
        );
    }
}
