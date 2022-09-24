use cosmwasm_std::{coin, coins, to_binary, Addr, BankMsg, CosmosMsg, Empty};
use cw_authorizations::msg::AuthoriazationExecuteMsg;
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

use cw_core::msg::ModuleInstantiateInfo;

use crate::msg::{ExecuteMsg, InstantiateMsg};

const CREATOR_ADDR: &str = "creator";

// Dao contract
fn cw_gov_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_core::contract::execute,
        cw_core::contract::instantiate,
        cw_core::contract::query,
    )
    .with_reply(cw_core::contract::reply);
    Box::new(contract)
}

// Authorization Contracts
fn cw_auth_manager() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

fn cw_whitelist_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        whitelist::contract::execute,
        whitelist::contract::instantiate,
        whitelist::contract::query,
    );
    Box::new(contract)
}

fn cw_message_filter_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        message_filter::contract::execute,
        message_filter::contract::instantiate,
        message_filter::contract::query,
    );
    Box::new(contract)
}

fn cw_satisfies_all_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        satisfies_all::contract::execute,
        satisfies_all::contract::instantiate,
        satisfies_all::contract::query,
    );
    Box::new(contract)
}

fn cw_satisfies_any_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        satisfies_any::contract::execute,
        satisfies_any::contract::instantiate,
        satisfies_any::contract::query,
    )
    .with_reply(satisfies_any::contract::reply);
    Box::new(contract)
}

fn instantiate_dao(app: &mut App, auth_module_code_id: u64) -> Addr {
    let core_contract_id = app.store_code(cw_gov_contract());

    let instantiate_core = cw_core::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: auth_module_code_id,
            msg: to_binary(&InstantiateMsg {}).unwrap(),
            admin: cw_core::msg::Admin::None {},
            label: "Fake Voring Module that shouldn't work".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: auth_module_code_id,
            label: "DAO DAO governance module.".to_string(),
            admin: cw_core::msg::Admin::CoreContract {},
            msg: to_binary(&InstantiateMsg {}).unwrap(),
        }],
        initial_items: None,
    };

    let core_addr = app
        .instantiate_contract(
            core_contract_id,
            Addr::unchecked(CREATOR_ADDR),
            &instantiate_core,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    core_addr
}

fn build_auth_dao() -> (App, Addr, Addr) {
    let init_funds = vec![coin(1000000, "juno"), coin(100, "other")];
    let mut app = App::new(|router, _, storage| {
        // initialization moved to App construction
        router
            .bank
            .init_balance(storage, &Addr::unchecked("McDuck"), init_funds)
            .unwrap();
    });

    // Create a proposal manager (gov module)
    let govmod_id = app.store_code(cw_auth_manager());

    // Create the DAO (core)
    let core_addr = instantiate_dao(&mut app, govmod_id);

    // Let's give the dao some funds
    let amount = coins(100000, "juno");
    let bank = BankMsg::Send {
        to_address: core_addr.to_string(),
        amount,
    };
    let msg: CosmosMsg = bank.clone().into();
    app.execute_multi(Addr::unchecked("McDuck"), vec![msg])
        .unwrap();

    // A dao can have several proposal/gov modules. Get the first one. This is our auth manager
    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr.clone(), &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let auth_manager = proposal_modules.into_iter().next().unwrap();
    (app, core_addr, auth_manager)
}

#[test]
fn test_direct_authorizations() {
    let (mut app, core_addr, auth_manager) = build_auth_dao();

    // Create the message that will go through the auth
    let amount = coins(1234, "juno");
    let bank = BankMsg::Send {
        to_address: "other_addr".to_string(),
        amount,
    };
    let msg: CosmosMsg = bank.clone().into();

    // Try to execute the message when no auths are configured
    app.execute_contract(
        Addr::unchecked("Evil sender"),
        auth_manager.clone(),
        &AuthoriazationExecuteMsg::Extension(ExecuteMsg::Execute {
            msgs: vec![msg.clone()],
        }),
        &[],
    )
    .unwrap_err(); // This should fail

    // Adding a simple authorization contract
    let whitelist_id = app.store_code(cw_whitelist_contract());
    let whitelist_addr = app
        .instantiate_contract(
            whitelist_id,
            Addr::unchecked("Shouldn't matter"),
            &whitelist::msg::InstantiateMsg {
                dao: core_addr.clone(),
            },
            &[],
            "Whitelist auth",
            None,
        )
        .unwrap();

    // Only the dao can add authorizations to the whitelist
    let whitelisted_sender = Addr::unchecked("whitelisted_addr");
    let _err = app
        .execute_contract(
            Addr::unchecked("Anyone"),
            whitelist_addr.clone(),
            &whitelist::msg::ExecuteMsg::Allow {
                addr: whitelisted_sender.to_string(),
            },
            &[],
        )
        .unwrap_err(); // This fails!

    app.execute_contract(
        Addr::unchecked(core_addr.clone()), // Cheating here. This should go through a proposal
        whitelist_addr.clone(),
        &AuthoriazationExecuteMsg::Extension(whitelist::msg::ExecuteMsg::Allow {
            addr: whitelisted_sender.to_string(),
        }),
        &[],
    )
    .unwrap(); // The address has been whitelisted

    // Create an *and* contract so that we can build more complex auths
    let and_id = app.store_code(cw_satisfies_all_contract());
    let and_addr = app
        .instantiate_contract(
            and_id,
            Addr::unchecked("Shouldn't matter"),
            &satisfies_all::msg::InstantiateMsg {
                admin: core_addr.clone(),
                parent: core_addr.clone(),
                children: vec![whitelist_addr.clone()],
            },
            &[],
            "And auth",
            None,
        )
        .unwrap();

    // Add the whitelist to the list of auths in the *and* contract
    app.execute_contract(
        Addr::unchecked(core_addr.clone()), // Cheating here. This should go through a proposal
        and_addr.clone(),
        &AuthoriazationExecuteMsg::Extension(satisfies_all::msg::ExecuteMsg::AddChild {
            addr: whitelist_addr.clone(),
        }),
        &[],
    )
    .unwrap();

    // Add an "any" contract as entrypoint
    let auth_entrypoint_id = app.store_code(cw_satisfies_any_contract());
    let auth_entrypoint_addr = app
        .instantiate_contract(
            auth_entrypoint_id,
            Addr::unchecked("Shouldn't matter"),
            &satisfies_any::msg::InstantiateMsg {
                admin: core_addr.clone(),
                parent: core_addr.clone(),
                children: vec![],
            },
            &[],
            "Entrypoint (any suba_uth)",
            None,
        )
        .unwrap();

    app.execute_contract(
        Addr::unchecked(core_addr.clone()), // Cheating here. This should go through a proposal
        auth_manager.clone(),
        &AuthoriazationExecuteMsg::Extension(ExecuteMsg::SetAuthorization {
            auth_contract: auth_entrypoint_addr.clone(),
        }),
        &[],
    )
    .unwrap();

    // Add the and to the any. This is all very confusing and should be simplified.
    app.execute_contract(
        Addr::unchecked(core_addr.clone()), // Cheating here. This should go through a proposal
        auth_entrypoint_addr.clone(),
        &AuthoriazationExecuteMsg::Extension(satisfies_any::msg::ExecuteMsg::AddChild {
            addr: and_addr.clone(),
        }),
        &[],
    )
    .unwrap();

    // Execute the proposal by someone who is not whitelisted
    app.execute_contract(
        Addr::unchecked("RaNdO"),
        auth_manager.clone(),
        &AuthoriazationExecuteMsg::Extension(ExecuteMsg::Execute {
            msgs: vec![msg.clone()],
        }),
        &[],
    )
    .unwrap_err();

    // Execute the proposal by someone who is whitelisted
    app.execute_contract(
        whitelisted_sender.clone(),
        auth_manager.clone(),
        &AuthoriazationExecuteMsg::Extension(ExecuteMsg::Execute { msgs: vec![msg] }),
        &[],
    )
    .unwrap();

    // Adding a filtering authorization contract
    let message_filter_id = app.store_code(cw_message_filter_contract());
    let message_filter_addr = app
        .instantiate_contract(
            message_filter_id,
            Addr::unchecked("Shouldn't matter"),
            &message_filter::msg::InstantiateMsg {
                parent: core_addr.clone(),
                kind: message_filter::state::Kind::Allow {},
            },
            &[],
            "Allow some message types - auth",
            None,
        )
        .unwrap();

    app.execute_contract(
        Addr::unchecked(core_addr.clone()), // Cheating here. This should go through a proposal
        and_addr.clone(),
        &AuthoriazationExecuteMsg::Extension(satisfies_all::msg::ExecuteMsg::AddChild {
            addr: message_filter_addr.clone(),
        }),
        &[],
    )
    .unwrap();

    // An employee can send transactions but only on of a specific token
    let employee_addr = Addr::unchecked("employee");
    app.execute_contract(
        Addr::unchecked(core_addr.clone()), // Cheating here. This should go through a proposal
        message_filter_addr.clone(),
        &AuthoriazationExecuteMsg::Extension(message_filter::msg::ExecuteMsg::AddAuthorization {
            addr: employee_addr.clone(),
            msg: r#"{"bank": {"send": {"to_address": {}, "amount": [{"denom": "juno", "amount": {}}]}}}"#.to_string(),
        }),
        &[],
    )
    .unwrap();

    // Create a proposal to spend some tokens
    let amount = coins(1234, "juno");
    let bank = BankMsg::Send {
        to_address: "other_addr".to_string(),
        amount,
    };
    let msg: CosmosMsg = bank.clone().into();

    // Someone without bank permissions tries to execute the proposal
    app.execute_contract(
        whitelisted_sender.clone(),
        auth_manager.clone(),
        &AuthoriazationExecuteMsg::Extension(ExecuteMsg::Execute {
            msgs: vec![msg.clone()],
        }),
        &[],
    )
    .unwrap_err(); // This should fail

    // The employee tries to execute the proposal... but they're not whitelisted!
    app.execute_contract(
        employee_addr.clone(),
        auth_manager.clone(),
        &AuthoriazationExecuteMsg::Extension(ExecuteMsg::Execute {
            msgs: vec![msg.clone()],
        }),
        &[],
    )
    .unwrap_err(); // This should fail

    // Whitelist the employee
    app.execute_contract(
        Addr::unchecked(core_addr.clone()), // Cheating here. This should go through a proposal or done by an authorized user
        whitelist_addr.clone(),
        &AuthoriazationExecuteMsg::Extension(whitelist::msg::ExecuteMsg::Allow {
            addr: employee_addr.to_string(),
        }),
        &[],
    )
    .unwrap(); // The address has been whitelisted

    // The employee tries to execute the proposal again. This time after being whitelisted
    app.execute_contract(
        employee_addr.clone(),
        auth_manager.clone(),
        &AuthoriazationExecuteMsg::Extension(ExecuteMsg::Execute {
            msgs: vec![msg.clone()],
        }),
        &[],
    )
    .unwrap(); // This should work!

    // Create a new proposal to spend some that don't match the employee's auth
    let amount = coins(1, "other");
    let bank = BankMsg::Send {
        to_address: "other_addr".to_string(),
        amount,
    };
    let msg: CosmosMsg = bank.clone().into();

    // The employee tries to execute the new proposal. This should fail because the coins aren't what the auth allows them
    app.execute_contract(
        employee_addr.clone(),
        auth_manager.clone(),
        &AuthoriazationExecuteMsg::Extension(ExecuteMsg::Execute {
            msgs: vec![msg.clone()],
        }),
        &[],
    )
    .unwrap_err(); // This should fail!
}
