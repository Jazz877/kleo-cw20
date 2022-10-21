// fn register_proposal_hook(app: &mut App, proposal_contract_addr: Addr, vesting_contract_addr: Addr) {
//     let _ = app.execute_contract(
//         Addr::unchecked(OWNER.to_string()),
//         proposal_contract_addr.clone(),
//         &cw_proposal_single::msg::ExecuteMsg::AddProposalHook {
//             address: vesting_contract_addr.clone().into_string(),
//         },
//         &vec![],
//     );
// }

// let proposal_contract_addr = instantiate_proposal(
// &mut app,
// Threshold::AbsolutePercentage {
// percentage: PercentageThreshold::Majority{},
// },
// Duration::Height(5),
// true,
// false,
// );
//
// fn instantiate_proposal(app: &mut App, threshold: Threshold, max_voting_period: Duration, only_members_execute: bool, allow_revoting: bool) -> Addr {
//     let proposal_code_id = app.store_code(contract_single_proposal());
//     let msg = cw_proposal_single::msg::InstantiateMsg {
//         threshold,
//         max_voting_period,
//         min_voting_period: None,
//         only_members_execute,
//         allow_revoting,
//         deposit_info: None,
//     };
//     app.instantiate_contract(proposal_code_id, Addr::unchecked(OWNER), &msg, &[], "proposal", None)
//         .unwrap()
// }
