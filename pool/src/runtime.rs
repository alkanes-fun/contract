












use crate::library::{Sqrt, StorableU256, BPS, PROTOCOL_FEE_SHARE_BPS, U256};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_std_factory_support::MintableToken;
use alkanes_std_maga_common::{expired, id_from_storage, pack_id, pack_u128s, swap_out, BytesPointer};
use alkanes_support::{
    context::Context,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};
use anyhow::{anyhow, Result};
use metashrew_support::index_pointer::KeyValuePointer;

pub const NAME: &str = "MAGA LP";
pub const SYMBOL: &str = "MAGA-LP";

pub const CONFIG_VERSION: u128 = 1;

fn p(k: &str) -> StoragePointer {
    StoragePointer::from_keyword(k)
}
fn get(k: &str) -> u128 {
    p(k).get_value::<u128>()
}
fn set(k: &str, v: u128) {
    p(k).set_value::<u128>(v)
}
fn id(k: &str) -> Result<AlkaneId> {
    id_from_storage(&p(k).get_bytes())
}
fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or_else(|| anyhow!("amount overflow"))
}
fn sub(a: u128, b: u128) -> Result<u128> {
    a.checked_sub(b).ok_or_else(|| anyhow!("amount underflow"))
}

fn mul_div(a: u128, b: u128, d: u128) -> Result<u128> {
    if d == 0 {
        return Err(anyhow!("division by zero"));
    }
    Ok((U256::from(a) * U256::from(b) / U256::from(d)).try_into()?)
}

fn mul_div_up(a: u128, b: u128, d: u128) -> Result<u128> {
    if d == 0 {
        return Err(anyhow!("division by zero"));
    }
    let n = U256::from(a) * U256::from(b);
    let d = U256::from(d);
    Ok(((n + d - U256::ONE) / d).try_into()?)
}
fn root_k(r0: u128, r1: u128) -> U256 {
    (U256::from(r0) * U256::from(r1)).sqrt()
}

fn swap_fee(amount: u128, bps: u128) -> Result<u128> {
    let base = (amount / BPS).checked_mul(bps).ok_or_else(|| anyhow!("fee overflow"))?;
    let r = (amount % BPS) * bps;
    add(base, r / BPS + u128::from(r % BPS != 0))
}

fn amount_of(parcel: &AlkaneTransferParcel, asset: &AlkaneId) -> Result<u128> {
    parcel.0.iter().filter(|t| t.id == *asset).try_fold(0u128, |n, t| add(n, t.value))
}

fn others(parcel: &AlkaneTransferParcel, ids: &[AlkaneId]) -> AlkaneTransferParcel {
    AlkaneTransferParcel(parcel.0.iter().filter(|t| !ids.contains(&t.id)).cloned().collect())
}


pub struct Settlement {

    pub minted: u128,

    pub cut: u128,
}

pub trait AMMPoolBase: MintableToken + AlkaneResponder {

    fn claimable_fees(&self) -> u128 {
        get("/claimablefees")
    }
    fn set_claimable_fees(&self, v: u128) {
        set("/claimablefees", v)
    }
    fn k_last(&self) -> U256 {
        p("/klast").get_value::<StorableU256>().into()
    }
    fn set_k_last(&self, v: U256) {
        p("/klast").set_value::<StorableU256>(v.into());
    }
    fn reserves(&self) -> (u128, u128) {
        (get("/r0"), get("/r1"))
    }
    fn set_reserves(&self, r0: u128, r1: u128) {
        set("/r0", r0);
        set("/r1", r1);
    }

    fn write_k_last(&self) {
        let (r0, r1) = self.reserves();
        self.set_k_last(U256::from(r0) * U256::from(r1));
    }

    fn asset(&self, n: u128) -> Result<AlkaneId> {
        match n {
            0 => id("/alkane/0"),
            1 => id("/alkane/1"),
            _ => Err(anyhow!("asset index must be 0 or 1")),
        }
    }
    fn require_initialized(&self) -> Result<()> {
        if get("/ready") != 1 {
            return Err(anyhow!("pool not initialized"));
        }
        Ok(())
    }
    fn require_bootstrapped(&self) -> Result<()> {
        self.require_initialized()?;
        if get("/bootstrapped") != 1 {
            return Err(anyhow!("pool not bootstrapped"));
        }
        Ok(())
    }
    fn require_not_paused(&self) -> Result<()> {
        if get("/paused") != 0 {
            return Err(anyhow!("pool paused"));
        }
        Ok(())
    }

    fn require_platform(&self, context: &Context) -> Result<()> {
        let auth = id("/platform")?;
        if !context.incoming_alkanes.0.iter().any(|t| t.id == auth && t.value > 0) {
            return Err(anyhow!("requires platform authority asset"));
        }
        Ok(())
    }


    fn settlement(&self) -> Result<Settlement> {
        let (r0, r1) = self.reserves();
        let root_k = root_k(r0, r1);
        if root_k.is_zero() {
            return Err(anyhow!("empty reserves"));
        }
        let total_supply = self.total_supply();

        let mut minted = 0u128;
        let k_last = self.k_last();
        if !k_last.is_zero() {
            let root_k_last = k_last.sqrt();
            if root_k > root_k_last {
                let numerator = U256::from(total_supply) * (root_k - root_k_last);
                let root_k_fee_adj = root_k * U256::from(BPS - PROTOCOL_FEE_SHARE_BPS)
                    / U256::from(PROTOCOL_FEE_SHARE_BPS);
                let denominator = root_k_fee_adj + root_k_last;
                minted = (numerator / denominator).try_into()?;
            }
        }


        let supply = U256::from(add(total_supply, minted)?);
        let keep = (U256::from(get("/principal")) * supply + root_k - U256::ONE) / root_k;
        let locked = U256::from(get("/locked"));
        let cut = if locked > keep { (locked - keep).try_into()? } else { 0 };
        Ok(Settlement { minted, cut })
    }


    fn checkpoint(&self) -> Result<()> {
        let s = self.settlement()?;
        if s.minted > 0 {
            self.increase_total_supply(s.minted)?;
        }
        set("/locked", sub(get("/locked"), s.cut)?);
        self.set_claimable_fees(add(add(self.claimable_fees(), s.minted)?, s.cut)?);
        self.write_k_last();
        Ok(())
    }


    fn init_pool(
        &self,
        project: u128,
        launch: AlkaneId,
        quote: AlkaneId,
        registry: AlkaneId,
        platform_auth: AlkaneId,
        swap_fee_bps: u128,
    ) -> Result<CallResponse> {
        self.observe_initialization()?;
        let c = self.context()?;
        if registry == AlkaneId::default() || c.caller != registry {
            return Err(anyhow!("initializer must be registry"));
        }
        for v in [launch, quote, platform_auth] {
            if v == AlkaneId::default() || v == c.myself {
                return Err(anyhow!("invalid config asset"));
            }
        }
        if platform_auth == launch || platform_auth == quote || launch == quote || swap_fee_bps >= BPS {
            return Err(anyhow!("invalid pool config"));
        }
        p("/alkane/0").set_bytes(pack_id(&quote));
        p("/alkane/1").set_bytes(pack_id(&launch));
        p("/registry").set_bytes(pack_id(&registry));
        p("/platform").set_bytes(pack_id(&platform_auth));
        set("/project", project);
        set("/fee_bps", swap_fee_bps);
        self.set_name_and_symbol_str(NAME.to_string(), SYMBOL.to_string());
        self.set_k_last(U256::ZERO);
        set("/ready", 1);
        let mut r = CallResponse::forward(&c.incoming_alkanes);
        r.data = pack_id(&c.myself);
        Ok(r)
    }






    fn add_liquidity(&self, min_lp_out: u128, valid_until: u128) -> Result<CallResponse> {
        self.require_initialized()?;
        if expired(self.height() as u128, valid_until) {
            return Err(anyhow!("add liquidity expired"));
        }
        let c = self.context()?;
        let (quote, token) = (self.asset(0)?, self.asset(1)?);
        let a = amount_of(&c.incoming_alkanes, &quote)?;
        let b = amount_of(&c.incoming_alkanes, &token)?;
        let rest = others(&c.incoming_alkanes, &[quote, token]);
        if get("/bootstrapped") == 0 {
            if c.caller != token {
                return Err(anyhow!("bootstrap requires bound launch"));
            }
            if !rest.0.is_empty() {
                return Err(anyhow!("unexpected bootstrap asset"));
            }
            if a == 0 || b == 0 {
                return Err(anyhow!("both reserves must be positive"));
            }
            let x: u128 = root_k(a, b).try_into()?;
            if x == 0 {
                return Err(anyhow!("insufficient liquidity minted"));
            }
            self.set_reserves(a, b);
            self.set_total_supply(x);
            set("/locked", x);
            set("/principal", x);
            set("/bootstrapped", 1);
            self.write_k_last();
            let mut r = CallResponse::default();
            r.data = pack_u128s(&[x, a, b]);
            return Ok(r);
        }
        self.require_not_paused()?;
        if a == 0 || b == 0 {
            return Err(anyhow!("both assets required"));
        }
        self.checkpoint()?;
        let s = self.total_supply();
        let (r0, r1) = self.reserves();
        let x = mul_div(a, s, r0)?.min(mul_div(b, s, r1)?);
        if x == 0 {
            return Err(anyhow!("insufficient liquidity minted"));
        }
        if x < min_lp_out {
            return Err(anyhow!("slippage"));
        }
        let used0 = mul_div_up(x, r0, s)?;
        let used1 = mul_div_up(x, r1, s)?;
        self.set_reserves(add(r0, used0)?, add(r1, used1)?);
        let mut r = CallResponse::default();
        r.alkanes = rest;
        if a > used0 {
            r.alkanes.pay(AlkaneTransfer { id: quote, value: a - used0 });
        }
        if b > used1 {
            r.alkanes.pay(AlkaneTransfer { id: token, value: b - used1 });
        }
        r.alkanes.pay(self.mint(&c, x)?);
        self.write_k_last();
        r.data = pack_u128s(&[x, used0, used1]);
        Ok(r)
    }




    fn withdraw_and_burn(&self, min_out_0: u128, min_out_1: u128, valid_until: u128) -> Result<CallResponse> {
        self.require_bootstrapped()?;
        if expired(self.height() as u128, valid_until) {
            return Err(anyhow!("withdraw expired"));
        }
        let c = self.context()?;
        let x = amount_of(&c.incoming_alkanes, &c.myself)?;
        if x == 0 {
            return Err(anyhow!("no LP sent"));
        }
        self.checkpoint()?;
        let circulating = sub(sub(self.total_supply(), get("/locked"))?, self.claimable_fees())?;
        if x > circulating {
            return Err(anyhow!("burn exceeds circulating LP"));
        }
        let (out0, out1) = self.burn(x)?;
        if out0 == 0 && out1 == 0 {
            return Err(anyhow!("insufficient liquidity burned"));
        }
        if out0 < min_out_0 || out1 < min_out_1 {
            return Err(anyhow!("slippage"));
        }
        let mut r = CallResponse::default();
        r.alkanes = others(&c.incoming_alkanes, &[c.myself]);
        self.pay_assets(&mut r, out0, out1)?;
        r.data = pack_u128s(&[out0, out1]);
        Ok(r)
    }




    fn burn(&self, x: u128) -> Result<(u128, u128)> {
        let s = self.total_supply();
        let (r0, r1) = self.reserves();
        let out0 = mul_div(x, r0, s)?;
        let out1 = mul_div(x, r1, s)?;
        self.set_reserves(r0 - out0, r1 - out1);
        self.decrease_total_supply(x)?;
        self.write_k_last();
        Ok((out0, out1))
    }

    fn pay_assets(&self, r: &mut CallResponse, out0: u128, out1: u128) -> Result<()> {
        if out0 > 0 {
            r.alkanes.pay(AlkaneTransfer { id: self.asset(0)?, value: out0 });
        }
        if out1 > 0 {
            r.alkanes.pay(AlkaneTransfer { id: self.asset(1)?, value: out1 });
        }
        Ok(())
    }


    fn pricing(&self, asset_in: u128, amount: u128) -> Result<(u128, u128)> {
        self.asset(asset_in)?;
        self.require_bootstrapped()?;
        self.require_not_paused()?;
        let f = swap_fee(amount, get("/fee_bps"))?;
        let effective = sub(amount, f)?;
        if effective == 0 {
            return Err(anyhow!("zero effective input"));
        }
        let (r0, r1) = self.reserves();
        let (rin, rout) = if asset_in == 0 { (r0, r1) } else { (r1, r0) };
        add(rin, amount)?;
        let out = swap_out(rout, effective, rin)?;
        if out == 0 || out >= rout {
            return Err(anyhow!("unrepresentable output"));
        }
        Ok((out, f))
    }



    fn swap(&self, asset_in: u128, min_out: u128, valid_until: u128) -> Result<CallResponse> {
        if expired(self.height() as u128, valid_until) {
            return Err(anyhow!("swap expired"));
        }
        let c = self.context()?;
        let input = self.asset(asset_in)?;
        let output = self.asset(1 - asset_in)?;
        let amount = amount_of(&c.incoming_alkanes, &input)?;
        let (out, f) = self.pricing(asset_in, amount)?;
        if out < min_out {
            return Err(anyhow!("slippage"));
        }
        let (r0, r1) = self.reserves();
        if asset_in == 0 {
            self.set_reserves(add(r0, amount)?, r1 - out);
        } else {
            self.set_reserves(r0 - out, add(r1, amount)?);
        }
        let mut r = CallResponse::default();
        r.alkanes = others(&c.incoming_alkanes, &[input]);
        r.alkanes.pay(AlkaneTransfer { id: output, value: out });
        r.data = pack_u128s(&[out, f]);
        Ok(r)
    }

    fn set_paused(&self, paused: u128) -> Result<CallResponse> {
        self.require_initialized()?;
        let c = self.context()?;
        self.require_platform(&c)?;
        if paused > 1 {
            return Err(anyhow!("invalid pause transition"));
        }
        set("/paused", paused);
        Ok(CallResponse::forward(&c.incoming_alkanes))
    }


    fn deposit_unclassified(&self) -> Result<CallResponse> {
        self.require_initialized()?;
        let c = self.context()?;
        let mut r = CallResponse::default();
        r.alkanes = others(&c.incoming_alkanes, &[self.asset(0)?, self.asset(1)?]);
        Ok(r)
    }






    fn collect_fees(&self, burn: u128) -> Result<CallResponse> {
        self.require_bootstrapped()?;
        let c = self.context()?;
        self.require_platform(&c)?;
        if burn > 1 {
            return Err(anyhow!("burn must be 0 or 1"));
        }
        self.checkpoint()?;
        let x = self.claimable_fees();
        self.set_claimable_fees(0);
        set("/paid", add(get("/paid"), x)?);
        let mut r = CallResponse::forward(&c.incoming_alkanes);
        let (out0, out1) = if burn == 1 && x > 0 { self.burn(x)? } else { (0, 0) };
        if burn == 1 {
            self.pay_assets(&mut r, out0, out1)?;
        } else if x > 0 {
            r.alkanes.pay(AlkaneTransfer { id: c.myself, value: x });
        }
        r.data = pack_u128s(&[x, out0, out1]);
        Ok(r)
    }

    fn forward_incoming(&self) -> Result<CallResponse> {
        let context = self.context()?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }


    fn words(&self, words: &[u128]) -> Result<CallResponse> {
        let mut r = CallResponse::forward(&self.context()?.incoming_alkanes);
        r.data = pack_u128s(words);
        Ok(r)
    }
    fn get_total_fee(&self) -> Result<CallResponse> {
        self.words(&[get("/fee_bps")])
    }

    fn get_state(&self) -> Result<CallResponse> {
        self.require_initialized()?;
        let (r0, r1) = self.reserves();
        self.words(&[r0, r1, self.total_supply(), get("/locked"), get("/principal"),
            self.claimable_fees(), get("/bootstrapped"), get("/paused")])
    }

    fn quote(&self, asset_in: u128, amount: u128) -> Result<CallResponse> {
        let (out, f) = self.pricing(asset_in, amount)?;
        self.words(&[out, f])
    }

    fn get_fees(&self) -> Result<CallResponse> {
        self.require_initialized()?;
        if get("/bootstrapped") == 0 {
            return self.words(&[0]);
        }
        let s = self.settlement()?;
        self.words(&[add(add(self.claimable_fees(), s.minted)?, s.cut)?])
    }
    fn get_paid(&self) -> Result<CallResponse> {
        self.require_initialized()?;
        self.words(&[get("/paid")])
    }

    fn get_config(&self) -> Result<CallResponse> {
        self.require_initialized()?;
        let mut r = self.words(&[CONFIG_VERSION, get("/project"), get("/fee_bps")])?;
        for k in ["/alkane/1", "/alkane/0", "/registry", "/platform"] {
            r.data.extend(p(k).get_bytes());
        }
        Ok(r)
    }

    fn audit(&self, asset: u128) -> Result<CallResponse> {
        self.require_initialized()?;
        let a = self.asset(asset)?;
        let (r0, r1) = self.reserves();
        let reserve = if asset == 0 { r0 } else { r1 };
        let balance = self.balance(&self.context()?.myself, &a);
        let surplus = balance.checked_sub(reserve).ok_or_else(|| anyhow!("insolvent accounting"))?;
        self.words(&[reserve, balance, surplus])
    }
    fn get_reserves(&self) -> Result<CallResponse> {
        let (r0, r1) = self.reserves();
        self.words(&[r0, r1])
    }
    fn get_name(&self) -> Result<CallResponse> {
        let mut r = CallResponse::forward(&self.context()?.incoming_alkanes);
        r.data = NAME.as_bytes().to_vec();
        Ok(r)
    }
    fn get_symbol(&self) -> Result<CallResponse> {
        let mut r = CallResponse::forward(&self.context()?.incoming_alkanes);
        r.data = SYMBOL.as_bytes().to_vec();
        Ok(r)
    }
    fn get_total_supply(&self) -> Result<CallResponse> {
        self.words(&[self.total_supply()])
    }
}
