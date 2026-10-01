









































use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::storage::StoragePointer;
use alkanes_runtime::{declare_alkane, message::MessageDispatch};
use alkanes_support::{
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};
use anyhow::{anyhow, Result};
use alkanes_std_maga_common::abi;
use alkanes_std_maga_common::codec::pack_u128s;
use alkanes_std_maga_common::math::{
    expired, min_tokens_required, quote_buy, split_fee, tokens_for_pool, tokens_sold_at,
    validate_curve_config, validate_max_buy, BPS,
};
use alkanes_std_maga_common::runtime::{pack_id, unpack_id, BytesPointer};
#[allow(unused_imports)]
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;


pub const CONTRACT_VERSION: u128 = 1;

#[derive(Default)]
pub struct MagaLaunch(());

#[derive(MessageDispatch)]
enum MagaLaunchMessage {
    #[opcode(0)]
    Initialize {
        project_id: u128,
        quote: AlkaneId,
        pool: AlkaneId,
        creator_auth: AlkaneId,
        platform_auth: AlkaneId,
        creator_bps: u128,
        open_height: u128,
        deadline: u128,
        supply: u128,
        target: u128,
        granularity: u128,
        name: String,
        symbol: String,


        max_buy: u128,
    },






    #[opcode(1)]
    Buy {
        min_tokens_out: u128,
        allow_partial: u128,
        valid_until: u128,
    },

    #[opcode(2)]
    ClaimCreatorFee,

    #[opcode(3)]
    ClaimPlatformFee,

    #[opcode(4)]
    Graduate,

    #[opcode(6)]
    #[returns(Vec<u8>)]
    PreviewBuy { budget: u128 },

    #[opcode(90)]
    #[returns(Vec<u8>)]
    GetState,

    #[opcode(92)]
    #[returns(Vec<u8>)]
    GetQuoteId,

    #[opcode(93)]
    #[returns(Vec<u8>)]
    GetAudit,

    #[opcode(96)]
    #[returns(u128)]
    GetVersion,

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    #[returns(String)]
    GetSymbol,

    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,
}


fn p_registry() -> StoragePointer {
    StoragePointer::from_keyword("/registry")
}
fn p_project() -> StoragePointer {
    StoragePointer::from_keyword("/project")
}

fn p_pool() -> StoragePointer {
    StoragePointer::from_keyword("/pool")
}

fn p_graduated() -> StoragePointer {
    StoragePointer::from_keyword("/graduated")
}

fn p_pool_tokens() -> StoragePointer {
    StoragePointer::from_keyword("/pool_tokens")
}
fn p_quote() -> StoragePointer {
    StoragePointer::from_keyword("/quote")
}
fn p_creator_auth() -> StoragePointer {
    StoragePointer::from_keyword("/creator_auth")
}
fn p_platform_auth() -> StoragePointer {
    StoragePointer::from_keyword("/platform_auth")
}
fn p_creator_bps() -> StoragePointer {
    StoragePointer::from_keyword("/creator_bps")
}
fn p_open_height() -> StoragePointer {
    StoragePointer::from_keyword("/open_height")
}
fn p_deadline() -> StoragePointer {
    StoragePointer::from_keyword("/deadline")
}
fn p_supply() -> StoragePointer {
    StoragePointer::from_keyword("/supply")
}
fn p_target() -> StoragePointer {
    StoragePointer::from_keyword("/target")
}
fn p_granularity() -> StoragePointer {
    StoragePointer::from_keyword("/granularity")
}
fn p_max_buy() -> StoragePointer {
    StoragePointer::from_keyword("/max_buy")
}
fn p_name() -> StoragePointer {
    StoragePointer::from_keyword("/name")
}
fn p_symbol() -> StoragePointer {
    StoragePointer::from_keyword("/symbol")
}

fn p_net() -> StoragePointer {
    StoragePointer::from_keyword("/net")
}

fn p_minted() -> StoragePointer {
    StoragePointer::from_keyword("/minted")
}

fn p_fee_creator() -> StoragePointer {
    StoragePointer::from_keyword("/fee_creator")
}

fn p_fee_platform() -> StoragePointer {
    StoragePointer::from_keyword("/fee_platform")
}

fn p_claimed_creator() -> StoragePointer {
    StoragePointer::from_keyword("/claimed_creator")
}

fn p_claimed_platform() -> StoragePointer {
    StoragePointer::from_keyword("/claimed_platform")
}

fn incoming_amount(incoming: &AlkaneTransferParcel, asset: &AlkaneId) -> u128 {
    incoming
        .0
        .iter()
        .filter(|t| &t.id == asset)
        .fold(0u128, |acc, t| acc.saturating_add(t.value))
}



fn incoming_minus(
    incoming: &AlkaneTransferParcel,
    asset: &AlkaneId,
    amount: u128,
) -> AlkaneTransferParcel {
    let mut to_take = amount;
    let mut out = AlkaneTransferParcel::default();
    for t in &incoming.0 {
        let take = if &t.id == asset {
            core::cmp::min(t.value, to_take)
        } else {
            0
        };
        to_take -= take;
        if t.value > take {
            out.0.push(AlkaneTransfer {
                id: t.id,
                value: t.value - take,
            });
        }
    }
    out
}

impl MagaLaunch {
    fn require_selling(&self) -> Result<()> {
        if p_graduated().get_value::<u128>() != 0 {
            return Err(anyhow!("graduated: the sale is closed"));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn initialize(
        &self,
        project_id: u128,
        quote: AlkaneId,
        pool: AlkaneId,
        creator_auth: AlkaneId,
        platform_auth: AlkaneId,
        creator_bps: u128,
        open_height: u128,
        deadline: u128,
        supply: u128,
        target: u128,
        granularity: u128,
        name: String,
        symbol: String,
        max_buy: u128,
    ) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        if creator_bps > BPS {
            return Err(anyhow!("creator bps out of range"));
        }
        if [&quote, &pool, &creator_auth, &platform_auth].contains(&&AlkaneId::default()) {
            return Err(anyhow!("zero alkane id in config"));
        }
        if pool == quote || pool == context.myself {
            return Err(anyhow!("pool must be a contract of its own"));
        }
        if quote == context.myself {
            return Err(anyhow!("quote asset cannot be the project token"));
        }


        for auth in [&creator_auth, &platform_auth] {
            if auth == &quote || auth == &context.myself {
                return Err(anyhow!("credential asset cannot be the quote asset or the project token"));
            }
        }
        if deadline < open_height {
            return Err(anyhow!("deadline before open height"));
        }
        validate_curve_config(supply, target, granularity)?;
        validate_max_buy(granularity, max_buy)?;

        p_registry().set_bytes(pack_id(&context.caller));
        p_project().set_value::<u128>(project_id);
        p_quote().set_bytes(pack_id(&quote));
        p_pool().set_bytes(pack_id(&pool));
        p_creator_auth().set_bytes(pack_id(&creator_auth));
        p_platform_auth().set_bytes(pack_id(&platform_auth));
        p_creator_bps().set_value::<u128>(creator_bps);
        p_open_height().set_value::<u128>(open_height);
        p_deadline().set_value::<u128>(deadline);
        p_supply().set_value::<u128>(supply);
        p_target().set_value::<u128>(target);
        p_granularity().set_value::<u128>(granularity);
        p_name().set_bytes(name.into_bytes());
        p_symbol().set_bytes(symbol.into_bytes());
        p_max_buy().set_value::<u128>(max_buy);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }




    fn buy(&self, min_tokens_out: u128, allow_partial: u128, valid_until: u128) -> Result<CallResponse> {
        let context = self.context()?;



        self.require_selling()?;
        let height = self.height() as u128;
        if height < p_open_height().get_value::<u128>() {
            return Err(anyhow!("sale not open"));
        }
        if expired(height, valid_until) {
            return Err(anyhow!("buy expired"));
        }



        let quote_asset = unpack_id(&p_quote().get_bytes())?;
        let budget = incoming_amount(&context.incoming_alkanes, &quote_asset);





        let net_raised = p_net().get_value::<u128>();
        let quote = quote_buy(
            net_raised,
            p_supply().get_value::<u128>(),
            p_target().get_value::<u128>(),
            p_granularity().get_value::<u128>(),
            p_max_buy().get_value::<u128>(),
            budget,
        )?;




        if quote.net < quote.requested_net && allow_partial == 0 {
            return Err(anyhow!("partial fill not allowed"));
        }
        if quote.tokens < min_tokens_required(&quote, min_tokens_out)? {
            return Err(anyhow!("slippage: tokens below minimum"));
        }



        let (creator_fee, platform_fee) =
            split_fee(quote.fee, p_creator_bps().get_value::<u128>())?;
        let net = net_raised + quote.net;
        p_net().set_value::<u128>(net);
        p_minted().set_value::<u128>(p_minted().get_value::<u128>() + quote.tokens);
        p_fee_creator().set_value::<u128>(p_fee_creator().get_value::<u128>() + creator_fee);
        p_fee_platform().set_value::<u128>(p_fee_platform().get_value::<u128>() + platform_fee);






        let target = p_target().get_value::<u128>();
        let graduated = if net == target {
            self.fund_pool(&context.myself, net, target)?;
            1
        } else {
            0
        };





        let mut response = CallResponse::default();
        response.alkanes = incoming_minus(
            &context.incoming_alkanes,
            &quote_asset,
            quote.net + quote.fee,
        );
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself,
            value: quote.tokens,
        });
        response.data = pack_u128s(&[quote.net, quote.fee, quote.tokens, quote.refund, graduated]);
        Ok(response)
    }











    fn graduate(&self) -> Result<CallResponse> {
        let context = self.context()?;



        if p_graduated().get_value::<u128>() != 0 {
            return Err(anyhow!("already graduated"));
        }
        let net = p_net().get_value::<u128>();
        if net == 0 {
            return Err(anyhow!("nothing raised"));
        }

        if self.height() as u128 <= p_deadline().get_value::<u128>() {
            return Err(anyhow!("graduation not open: deadline not passed"));
        }
        let target = p_target().get_value::<u128>();

        let receipt = self.fund_pool(&context.myself, net, target)?;



        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = pack_u128s(&receipt);
        Ok(response)
    }





    fn fund_pool(&self, myself: &AlkaneId, net: u128, target: u128) -> Result<[u128; 5]> {




        let pool_tokens = tokens_for_pool(net, p_supply().get_value::<u128>(), target)?;
        if pool_tokens == 0 {
            return Err(anyhow!("zero pool allocation"));
        }





        p_graduated().set_value::<u128>(1);
        p_pool_tokens().set_value::<u128>(pool_tokens);

        p_minted().set_value::<u128>(
            p_minted()
                .get_value::<u128>()
                .checked_add(pool_tokens)
                .ok_or_else(|| anyhow!("minted overflow"))?,
        );






        let quote_asset = unpack_id(&p_quote().get_bytes())?;
        let pool = unpack_id(&p_pool().get_bytes())?;
        let funding = AlkaneTransferParcel(vec![

            AlkaneTransfer {
                id: quote_asset,
                value: net,
            },


            AlkaneTransfer {
                id: *myself,
                value: pool_tokens,
            },
        ]);

        let receipt = self.call(&abi::pool::add_liquidity(&pool, 0, 0), &funding, self.fuel())?;




        let r = abi::pool::AddLiquidityReceipt::decode(&receipt.data)?;
        if r.lp == 0 || (r.used_quote, r.used_token) != (net, pool_tokens) || !receipt.alkanes.0.is_empty() {
            return Err(anyhow!("unexpected pool receipt"));
        }
        let locked_lp = r.lp;



        if self.balance(&pool, &quote_asset) < net || self.balance(&pool, myself) < pool_tokens {
            return Err(anyhow!("pool does not hold the reserves"));
        }
        if self.balance(myself, &pool) != 0 {
            return Err(anyhow!("launch received LP"));
        }
        Ok([net, pool_tokens, pool.block, pool.tx, locked_lp])
    }

    fn preview_buy(&self, budget: u128) -> Result<CallResponse> {
        self.require_selling()?;
        let quote = quote_buy(
            p_net().get_value::<u128>(),
            p_supply().get_value::<u128>(),
            p_target().get_value::<u128>(),
            p_granularity().get_value::<u128>(),
            p_max_buy().get_value::<u128>(),
            budget,
        )?;
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = pack_u128s(&[
            quote.requested_net,
            quote.net,
            quote.fee,
            quote.tokens,
            quote.refund,
        ]);
        Ok(response)
    }






    fn get_state(&self) -> Result<CallResponse> {
        let pool = unpack_id(&p_pool().get_bytes())?;
        let supply = p_supply().get_value::<u128>();
        let target = p_target().get_value::<u128>();
        let net = p_net().get_value::<u128>();
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = pack_u128s(&[
            CONTRACT_VERSION,
            p_project().get_value::<u128>(),
            net,
            target,
            supply,
            p_minted().get_value::<u128>(),
            tokens_sold_at(net, supply, target)?,
            p_granularity().get_value::<u128>(),
            p_open_height().get_value::<u128>(),
            p_deadline().get_value::<u128>(),
            p_creator_bps().get_value::<u128>(),
            p_fee_creator().get_value::<u128>(),
            p_fee_platform().get_value::<u128>(),
            p_graduated().get_value::<u128>(),
            p_pool_tokens().get_value::<u128>(),
            pool.block,
            pool.tx,
            p_max_buy().get_value::<u128>(),
        ]);
        Ok(response)
    }






    fn solvency(&self) -> Result<(u128, u128)> {
        let context = self.context()?;
        let quote_asset = unpack_id(&p_quote().get_bytes())?;

        let held = self
            .balance(&context.myself, &quote_asset)
            .checked_sub(incoming_amount(&context.incoming_alkanes, &quote_asset))
            .ok_or_else(|| anyhow!("held quote below incoming"))?;
        let raise_held = if p_graduated().get_value::<u128>() == 0 {
            p_net().get_value::<u128>()
        } else {
            0
        };
        let owed = raise_held
            + p_fee_creator().get_value::<u128>()
            + p_fee_platform().get_value::<u128>();
        let surplus = held
            .checked_sub(owed)
            .ok_or_else(|| anyhow!("insolvent: held quote below net + unclaimed fees"))?;
        Ok((held, surplus))
    }









    fn claim_fee(
        &self,
        credential: StoragePointer,
        mut bucket: StoragePointer,
        mut claimed: StoragePointer,
    ) -> Result<CallResponse> {
        let context = self.context()?;


        let credential_asset = unpack_id(&credential.get_bytes())?;
        if incoming_amount(&context.incoming_alkanes, &credential_asset) == 0 {
            return Err(anyhow!("claim requires the credential asset"));
        }



        self.solvency()?;


        let amount = bucket.get_value::<u128>();
        bucket.set_value::<u128>(0);
        claimed.set_value::<u128>(
            claimed
                .get_value::<u128>()
                .checked_add(amount)
                .ok_or_else(|| anyhow!("claimed total overflow"))?,
        );



        let mut response = CallResponse::forward(&context.incoming_alkanes);
        if amount > 0 {
            response.alkanes.0.push(AlkaneTransfer {
                id: unpack_id(&p_quote().get_bytes())?,
                value: amount,
            });
        }
        response.data = pack_u128s(&[amount]);
        Ok(response)
    }

    fn claim_creator_fee(&self) -> Result<CallResponse> {
        self.claim_fee(p_creator_auth(), p_fee_creator(), p_claimed_creator())
    }

    fn claim_platform_fee(&self) -> Result<CallResponse> {
        self.claim_fee(p_platform_auth(), p_fee_platform(), p_claimed_platform())
    }




    fn get_audit(&self) -> Result<CallResponse> {
        let (held, surplus) = self.solvency()?;
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = pack_u128s(&[
            p_net().get_value::<u128>(),
            p_fee_creator().get_value::<u128>(),
            p_fee_platform().get_value::<u128>(),
            held,
            surplus,
            p_claimed_creator().get_value::<u128>(),
            p_claimed_platform().get_value::<u128>(),
        ]);
        Ok(response)
    }

    fn get_quote_id(&self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = p_quote().get_bytes();
        Ok(response)
    }

    fn get_version(&self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = CONTRACT_VERSION.to_le_bytes().to_vec();
        Ok(response)
    }

    fn get_name(&self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = p_name().get_bytes();
        Ok(response)
    }

    fn get_symbol(&self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = p_symbol().get_bytes();
        Ok(response)
    }

    fn get_total_supply(&self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);
        response.data = p_minted().get_value::<u128>().to_le_bytes().to_vec();
        Ok(response)
    }
}

impl AlkaneResponder for MagaLaunch {}






















































declare_alkane! {
    impl AlkaneResponder for MagaLaunch {
        type Message = MagaLaunchMessage;
    }
}
