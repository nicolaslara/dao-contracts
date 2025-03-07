use cosmwasm_std::Addr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Group {
    pub name: String,
    pub addresses: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub groups: Option<Vec<Group>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Update {
        group: String,
        addresses_to_add: Option<Vec<String>>,
        addresses_to_remove: Option<Vec<String>>,
    },
    RemoveGroup {
        group: String,
    },
    UpdateOwner {
        owner: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Dump {},
    ListGroups {
        address: String,
        offset: Option<u32>,
        limit: Option<u32>,
    },
    ListAddresses {
        group: String,
        offset: Option<u32>,
        limit: Option<u32>,
    },
    IsAddressInGroup {
        address: String,
        group: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DumpResponse {
    pub groups: Vec<Group>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ListGroupsResponse {
    pub groups: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ListAddressesResponse {
    pub addresses: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IsAddressInGroupResponse {
    pub is_in_group: bool,
}
