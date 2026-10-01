
















use crate::codec::{encode_string, unpack_u128s};
use crate::runtime::id_words;
use alkanes_support::{cellpack::Cellpack, id::AlkaneId};
use anyhow::{anyhow, Result};

pub mod factory {
    use super::*;



    pub const CLONE_BLOCK: u128 = 5;

    pub fn clone(template: &AlkaneId, inputs: Vec<u128>) -> Cellpack {
        Cellpack {
            target: AlkaneId {
                block: CLONE_BLOCK,
                tx: template.tx,
            },
            inputs,
        }
    }
}

pub mod credential {
    use super::*;



    pub const INITIALIZE: u128 = 0;

    pub fn clone_and_initialize(template: &AlkaneId, name: &str, symbol: &str, amount: u128) -> Cellpack {
        let mut inputs = vec![INITIALIZE];
        inputs.extend(encode_string(name));
        inputs.extend(encode_string(symbol));
        inputs.push(amount);
        factory::clone(template, inputs)
    }
}

pub mod launch {
    use super::*;


    pub const INITIALIZE: u128 = 0;


    pub struct Init<'a> {
        pub project_id: u128,
        pub quote: AlkaneId,
        pub pool: AlkaneId,
        pub creator_auth: AlkaneId,
        pub platform_auth: AlkaneId,
        pub creator_bps: u128,
        pub open_height: u128,
        pub deadline: u128,
        pub supply: u128,
        pub target: u128,
        pub granularity: u128,
        pub name: &'a str,
        pub symbol: &'a str,
        pub max_buy: u128,
    }


    pub fn clone_and_initialize(template: &AlkaneId, init: &Init) -> Cellpack {
        let mut inputs = vec![INITIALIZE, init.project_id];
        inputs.extend(id_words(&init.quote));
        inputs.extend(id_words(&init.pool));
        inputs.extend(id_words(&init.creator_auth));
        inputs.extend(id_words(&init.platform_auth));
        inputs.extend([
            init.creator_bps,
            init.open_height,
            init.deadline,
            init.supply,
            init.target,
            init.granularity,
        ]);
        inputs.extend(encode_string(init.name));
        inputs.extend(encode_string(init.symbol));
        inputs.push(init.max_buy);
        factory::clone(template, inputs)
    }
}

pub mod pool {
    use super::*;


    pub const INIT_POOL: u128 = 0;

    pub const ADD_LIQUIDITY: u128 = 1;


    pub struct Init {
        pub project: u128,
        pub launch: AlkaneId,
        pub quote: AlkaneId,
        pub registry: AlkaneId,
        pub platform_auth: AlkaneId,
        pub swap_fee_bps: u128,
    }


    pub fn clone_and_initialize(template: &AlkaneId, init: &Init) -> Cellpack {
        let mut inputs = vec![INIT_POOL, init.project];
        inputs.extend(id_words(&init.launch));
        inputs.extend(id_words(&init.quote));
        inputs.extend(id_words(&init.registry));
        inputs.extend(id_words(&init.platform_auth));
        inputs.push(init.swap_fee_bps);
        factory::clone(template, inputs)
    }


    pub fn decode_init_receipt(data: &[u8]) -> Result<AlkaneId> {
        match unpack_u128s(data)?[..] {
            [block, tx] => Ok(AlkaneId { block, tx }),
            _ => Err(anyhow!("pool template reported an unexpected pool id")),
        }
    }


    pub fn add_liquidity(pool: &AlkaneId, min_lp_out: u128, valid_until: u128) -> Cellpack {
        Cellpack {
            target: *pool,
            inputs: vec![ADD_LIQUIDITY, min_lp_out, valid_until],
        }
    }



    #[derive(Debug, PartialEq, Eq)]
    pub struct AddLiquidityReceipt {
        pub lp: u128,
        pub used_quote: u128,
        pub used_token: u128,
    }

    impl AddLiquidityReceipt {
        pub fn decode(data: &[u8]) -> Result<Self> {
            match unpack_u128s(data)?[..] {
                [lp, used_quote, used_token] => Ok(Self { lp, used_quote, used_token }),
                _ => Err(anyhow!("unexpected pool receipt")),
            }
        }
    }
}
