use cosmwasm_std::{Empty, Addr, Uint128, coin};
use cw20::{Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg, BalanceResponse};
use cw_multi_test::{Contract, ContractWrapper, App, Executor, next_block};
use cw_proposal_single;
use cw_utils::Duration;
use voting::{Threshold, PercentageThreshold};
use crate::{contract, msg::{InstantiateMsg, ExecuteMsg, QueryMsg, VestingAccountResponse}, state::VestingData};

const OWNER: &str = "owner0000";
const INITIAL_BALANCE: u128 = 100_000_000;
const USER1: &str = "user0001";



pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

pub fn contract_vesting() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        contract::execute,
        contract::instantiate,
        contract::query,
    );
    Box::new(contract)
}

pub fn contract_single_proposal() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_proposal_single::contract::execute,
        cw_proposal_single::contract::instantiate,
        cw_proposal_single::contract::query,
    );
    Box::new(contract)
}

fn mock_app() -> App {
    let init_funds = vec![coin(20, "juno")];

    let app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked(&OWNER.to_string()), init_funds)
            .unwrap();
    });
    app
}

fn instantiate_cw20(app: &mut App) -> Addr {
    let cw20_code_id = app.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("Kleomedes"),
        symbol: String::from("KLEO"),
        decimals: 6,
        initial_balances: vec![
            Cw20Coin {
                address: OWNER.to_string(),
                amount: Uint128::new(INITIAL_BALANCE),
            }
        ],
        mint: None,
        marketing: None,
    };
    app.instantiate_contract(cw20_code_id, Addr::unchecked(OWNER), &msg, &[], "cw20", None)
        .unwrap()
}

fn instantiate_vesting(app: &mut App, token_address: Addr) -> Addr {
    let vesting_code_id = app.store_code(contract_vesting());
    let msg = InstantiateMsg {
        token_address: token_address,
        owner_address: Some(Addr::unchecked(OWNER)),
    };
    app.instantiate_contract(vesting_code_id, Addr::unchecked(OWNER), &msg, &[], "vesting", None)
        .unwrap()
}

fn instantiate_proposal(app: &mut App, threshold: Threshold, max_voting_period: Duration, only_members_execute: bool, allow_revoting: bool) -> Addr {
    let proposal_code_id = app.store_code(contract_single_proposal());
    let msg = cw_proposal_single::msg::InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute,
        allow_revoting,
        deposit_info: None,
    };
    app.instantiate_contract(proposal_code_id, Addr::unchecked(OWNER), &msg, &[], "proposal", None)
        .unwrap()
}

fn query_cw20_balance(app: &App, cw20_addr: Addr, address: Addr) -> Uint128 {
    let msg = Cw20QueryMsg::Balance {address: address.into_string()};
    let balance_response: BalanceResponse = app
        .wrap()
        .query_wasm_smart(cw20_addr, &msg).unwrap();

    balance_response.balance
}

fn register_proposal_hook(app: &mut App, proposal_contract_addr: Addr, vesting_contract_addr: Addr) {
    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        proposal_contract_addr.clone(),
        &cw_proposal_single::msg::ExecuteMsg::AddProposalHook {
            address: vesting_contract_addr.clone().into_string(),
        },
        &vec![],
    );
}

#[test]
fn simple_e2e_test() {
    let mut app = mock_app();
    let cw20_contract_addr = instantiate_cw20(&mut app);
    let vesting_contract_addr = instantiate_vesting(&mut app, cw20_contract_addr.clone());
    let proposal_contract_addr = instantiate_proposal(
        &mut app,
        Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority{},
        },
        Duration::Height(5),
        true,
        false,
    );

    // register hook for vesting contract on proposal contract
    register_proposal_hook(&mut app, proposal_contract_addr.clone(), vesting_contract_addr.clone());

    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        proposal_contract_addr.clone(),
        &cw_proposal_single::msg::ExecuteMsg::Propose {
            title: "new proposal 1".to_string(),
            description: "eeeeh".to_string(),
            msgs: vec![],
        },
        &vec![],
    );

    let initial_owner_balance = Uint128::new(INITIAL_BALANCE); 

    println!("{:?}", cw20_contract_addr.clone());
    println!("{:?}", vesting_contract_addr.clone());

    let initial_block_time = app.block_info().time;

    // move kleo on vesting contract
    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        cw20_contract_addr.clone(),
        &Cw20ExecuteMsg::Transfer {
            recipient: vesting_contract_addr.clone().into_string(),
            amount: Uint128::new(10_000_000u128),
        },
        &vec![],
    );

    let owner_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), Addr::unchecked(OWNER.to_string()));
    assert_eq!(initial_owner_balance.checked_sub(Uint128::new(10_000_000u128)).unwrap(), owner_balance);

    let vesting_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), vesting_contract_addr.clone());
    assert_eq!(Uint128::new(10_000_000u128), vesting_balance);

    // register vesting account
    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked(USER1.to_string()),
            vesting_amount: Uint128::new(10_000_000),
            start_time: initial_block_time,
            end_time: initial_block_time.plus_seconds(100),
        },
        &vec![],
    );

    // 5seconds more
    app.update_block(next_block);
    
    let msg = QueryMsg::VestingAccount {
        address: Addr::unchecked(USER1.to_string()), height: None,
    };

    let res: VestingAccountResponse = app.wrap().query_wasm_smart(vesting_contract_addr.clone(), &msg).unwrap();

    assert_eq!(
        VestingAccountResponse { 
            address: Addr::unchecked(USER1.to_string()),
            vestings: VestingData { 
                vesting_amount: Uint128::new(10_000_000u128), 
                vested_amount: Uint128::new(500_000u128), 
                claimable_amount: Uint128::new(500_000u128), 
                claimed_amount: Uint128::zero(), 
                start_time: initial_block_time, 
                end_time: initial_block_time.plus_seconds(100u64),
            } 
        },
        res
    );

    // user1 claims tokens
    let _ = app.execute_contract(
        Addr::unchecked(USER1.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::Claim {
            recipient: None,
        },
        &vec![],
    );

    let user1_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), Addr::unchecked(USER1.to_string()));
    assert_eq!(Uint128::new(500_000u128), user1_balance);

    // deregister user1

    //5seconds after
    app.update_block(next_block);

    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::DeregisterVestingAccount {
            address: Addr::unchecked(USER1.to_string()),
            vested_token_recipient: Some(Addr::unchecked(USER1.to_string())),
            left_vesting_token_recipient: Some(Addr::unchecked(OWNER.to_string())),
        },
        &vec![],
    );

    let user1_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), Addr::unchecked(USER1.to_string()));
    assert_eq!(Uint128::new(1_000_000u128), user1_balance);

    let owner_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), Addr::unchecked(OWNER.to_string()));
    assert_eq!(Uint128::new(99_000_000), owner_balance);
}