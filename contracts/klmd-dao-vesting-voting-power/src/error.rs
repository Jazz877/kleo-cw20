use cosmwasm_std::StdError;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Got a submessage reply with unknown id: {id}")]
    UnknownReplyId { id: u64 },

    #[error("Vesting contract token address does not match provided token address")]
    VestingContractMismatch {},

    #[error("Can not change the contract's vesting contract after it has been set")]
    DuplicateVestingContract {},

    #[error("Impossible to instantiate a new vesting contract")]
    VestingInstantiateError {},
}
