





use alkanes_runtime::{declare_alkane, message::MessageDispatch, runtime::AlkaneResponder};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use runtime::AMMPoolBase;
use alkanes_std_factory_support::MintableToken;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};

mod library;
mod runtime;

#[derive(MessageDispatch)]
pub enum AMMPoolMessage {

    #[opcode(0)]
    InitPool {
        project: u128,
        launch: AlkaneId,
        quote: AlkaneId,
        registry: AlkaneId,
        platform_auth: AlkaneId,
        swap_fee_bps: u128,
    },




    #[opcode(1)]
    AddLiquidity { min_lp_out: u128, valid_until: u128 },

    #[opcode(2)]
    WithdrawAndBurn { min_out_0: u128, min_out_1: u128, valid_until: u128 },


    #[opcode(3)]
    Swap { asset_in: u128, min_out: u128, valid_until: u128 },


    #[opcode(4)]
    SetPaused { paused: u128 },

    #[opcode(7)]
    DepositUnclassified,



    #[opcode(10)]
    CollectFees { burn: u128 },

    #[opcode(20)]
    #[returns(u128)]
    GetTotalFee {},

    #[opcode(50)]
    ForwardIncoming,

    #[opcode(90)]
    GetState,

    #[opcode(91)]
    Quote { asset_in: u128, amount: u128 },

    #[opcode(92)]
    GetFees,

    #[opcode(93)]
    GetPaid,

    #[opcode(94)]
    GetConfig,

    #[opcode(96)]
    Audit { asset: u128 },

    #[opcode(97)]
    #[returns(u128, u128)]
    GetReserves,

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    GetSymbol,

    #[opcode(101)]
    GetTotalSupply,
}

#[derive(Default)]
pub struct AMMPool();

impl MintableToken for AMMPool {}
impl AMMPoolBase for AMMPool {}

impl AlkaneResponder for AMMPool {}
declare_alkane! {
    impl AlkaneResponder for AMMPool {
        type Message = AMMPoolMessage;
    }
}
