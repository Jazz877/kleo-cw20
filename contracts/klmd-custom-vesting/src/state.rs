use cosmwasm_std::{Addr, BlockInfo, StdError, StdResult, Timestamp, Uint128, Uint64};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const ACCOUNTS: Map<&Addr, Account> = Map::new("accounts");
pub const BLOCK_TIME: Item<Uint64> = Item::new("block_time");
pub const TOKEN_ADDRESS: Item<Addr> = Item::new("token_address");
pub const OWNER_ADDRESS: Item<Addr> = Item::new("owner_address");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum PaymentStatus {
    Pending,
    Paid,
    Revoked,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Payment {
    pub amount: Uint128,
    pub timestamp: Timestamp,
    pub height: Uint64,
    pub status: PaymentStatus,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Account {
    pub address: Addr,
    pub vesting_amount: Uint128,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
    pub scheduled_payments: Vec<Payment>,
}

impl Account {
    pub fn validate(&self, block_info: &BlockInfo) -> StdResult<()> {
        if self.vesting_amount.is_zero() {
            return Err(StdError::generic_err("assert(vesting_amount > 0)"));
        }

        if self.start_time < block_info.time {
            return Err(StdError::generic_err("assert(start_time >= block_time)"));
        }

        if self.end_time < self.start_time {
            return Err(StdError::generic_err("assert(end_time >= start_time)"));
        }

        Ok(())
    }

    pub fn vested_amount(&self, block_info: &BlockInfo, height: Option<u64>) -> StdResult<Uint128> {
        let input_height = height.unwrap_or(block_info.height);
        let mut vested_amount = Uint128::zero();

        if block_info.time < self.start_time {
            return Ok(vested_amount);
        }


        for payment in &self.scheduled_payments {
            if payment.height.u64() <= input_height {
                vested_amount += payment.amount;
            }
        }

        Ok(vested_amount)
    }

    pub fn claimed_amount(&self, block_info: &BlockInfo, height: Option<u64>) -> StdResult<Uint128> {
        let input_height = height.unwrap_or(block_info.height);
        let mut claimed_amount = Uint128::zero();

        if block_info.time < self.start_time {
            return Ok(claimed_amount);
        }

        for payment in &self.scheduled_payments {
            if payment.status == PaymentStatus::Paid && payment.height.u64() <= input_height {
                claimed_amount += payment.amount;
            }
        }

        Ok(claimed_amount)
    }


    pub fn claimable_amount(&self, block_info: &BlockInfo, height: Option<u64>) -> StdResult<Uint128> {
        let input_height = height.unwrap_or(block_info.height);
        let mut claimable_amount = Uint128::zero();

        if block_info.time < self.start_time {
            return Ok(claimable_amount);
        }

        for payment in &self.scheduled_payments {
            if payment.status == PaymentStatus::Pending && payment.height.u64() <= input_height {
                claimable_amount += payment.amount;
            }
        }

        Ok(claimable_amount)
    }
}

pub fn convert_seconds_to_number_of_blocks(seconds: Uint64, block_time: Uint64) -> Uint64 {
    seconds.checked_div(block_time).unwrap_or(Uint64::zero())
}

pub fn compute_payments_for_time_interval(block_time: Uint64, block_info: &BlockInfo, start_time: Timestamp, end_time: Timestamp, vesting_amount: Uint128) -> Vec<Payment> {
    let block_seconds = block_time.checked_div(Uint64::new(1000)).unwrap_or(Uint64::zero());
    let start_block = block_info.height + convert_seconds_to_number_of_blocks(Uint64::new(start_time.seconds() - block_info.time.seconds()), block_seconds).u64();

    let delta_time = end_time.seconds() - start_time.seconds();
    let tot_number_of_block = convert_seconds_to_number_of_blocks(Uint64::new(delta_time), block_seconds);
    let amount_per_block = vesting_amount.checked_div(Uint128::from(tot_number_of_block)).unwrap_or(Uint128::zero());

    let mut payments: Vec<Payment> = Vec::new();
    for i in 0..tot_number_of_block.u64() {
        let payment = Payment {
            height: Uint64::new(start_block + i),
            amount: amount_per_block.clone(),
            timestamp: start_time.plus_seconds(block_seconds.checked_mul(Uint64::new(i)).unwrap_or(Uint64::zero()).u64()),
            status: PaymentStatus::Pending,
        };
        payments.push(payment);
    }
    payments
}
