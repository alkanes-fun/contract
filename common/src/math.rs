
















use anyhow::{anyhow, Result};
use ruint::aliases::U256;

pub const BPS: u128 = 10_000;
pub const FEE_DIVISOR: u128 = 100;

pub fn ceil_div(a: u128, b: u128) -> Result<u128> {
    if b == 0 {
        return Err(anyhow!("division by zero"));
    }
    let q = a / b;
    if a % b == 0 {
        Ok(q)
    } else {
        q.checked_add(1).ok_or_else(|| anyhow!("ceil overflow"))
    }
}


pub fn fee_on_net(b: u128) -> Result<u128> {
    ceil_div(b, FEE_DIVISOR)
}


pub fn fee_on_input(a: u128) -> Result<u128> {
    ceil_div(a, FEE_DIVISOR)
}


pub fn net_from_budget(budget: u128) -> Result<u128> {


    Ok(budget - ceil_div(budget, 101)?)
}


pub fn split_fee(fee: u128, creator_bps: u128) -> Result<(u128, u128)> {
    if creator_bps > BPS {
        return Err(anyhow!("creator bps out of range"));
    }


    let creator = (fee / BPS) * creator_bps + (fee % BPS) * creator_bps / BPS;
    Ok((creator, fee - creator))
}


pub fn tokens_sold_at(r: u128, c: u128, q: u128) -> Result<u128> {
    let three_r = r.checked_mul(3).ok_or_else(|| anyhow!("3r overflow"))?;
    let denom = q
        .checked_add(three_r)
        .ok_or_else(|| anyhow!("q+3r overflow"))?
        .checked_mul(5)
        .ok_or_else(|| anyhow!("denom overflow"))?;
    let numer = r
        .checked_mul(c)
        .ok_or_else(|| anyhow!("r*c overflow"))?
        .checked_mul(16)
        .ok_or_else(|| anyhow!("16cr overflow"))?;
    numer.checked_div(denom).ok_or_else(|| anyhow!("curve div"))
}


pub fn tokens_for_pool(r: u128, c: u128, q: u128) -> Result<u128> {
    let three_r = r.checked_mul(3).ok_or_else(|| anyhow!("3r overflow"))?;
    let base = q
        .checked_add(three_r)
        .ok_or_else(|| anyhow!("q+3r overflow"))?;
    let denom = base
        .checked_mul(base)
        .ok_or_else(|| anyhow!("denom sq overflow"))?
        .checked_mul(5)
        .ok_or_else(|| anyhow!("denom overflow"))?;
    let numer = r
        .checked_mul(c)
        .ok_or_else(|| anyhow!("r*c overflow"))?
        .checked_mul(q)
        .ok_or_else(|| anyhow!("rcq overflow"))?
        .checked_mul(16)
        .ok_or_else(|| anyhow!("16rcq overflow"))?;
    numer
        .checked_div(denom)
        .ok_or_else(|| anyhow!("pool alloc div"))
}



pub fn swap_out(reserve_out: u128, effective_in: u128, reserve_in: u128) -> Result<u128> {

    let denom = U256::from(reserve_in) + U256::from(effective_in);
    if denom == U256::ZERO {
        return Err(anyhow!("swap div"));
    }

    Ok((U256::from(reserve_out) * U256::from(effective_in) / denom).to::<u128>())
}



pub fn validate_curve_config(supply: u128, target: u128, granularity: u128) -> Result<()> {
    if supply == 0 || target == 0 || granularity == 0 {
        return Err(anyhow!("supply, target and granularity must be non-zero"));
    }
    if target % granularity != 0 {
        return Err(anyhow!("target must be a multiple of granularity"));
    }
    let sold = tokens_sold_at(target, supply, target)?;


    if tokens_for_pool(target, supply, target)? == 0 {
        return Err(anyhow!("pool allocation at target would be zero"));
    }

    if sold == tokens_sold_at(target - granularity, supply, target)? {
        return Err(anyhow!("last purchase unit would mint zero tokens"));
    }
    Ok(())
}



pub fn validate_max_buy(granularity: u128, max_buy: u128) -> Result<()> {
    if max_buy != 0 && max_buy % granularity != 0 {
        return Err(anyhow!("max buy must be a multiple of granularity"));
    }
    Ok(())
}





pub fn expired(height: u128, valid_until: u128) -> bool {
    valid_until != 0 && height > valid_until
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyQuote {

    pub requested_net: u128,

    pub net: u128,
    pub fee: u128,
    pub tokens: u128,

    pub refund: u128,
}





pub fn quote_buy(
    net_raised: u128,
    supply: u128,
    target: u128,
    granularity: u128,
    max_buy: u128,
    budget: u128,
) -> Result<BuyQuote> {
    let remaining = target
        .checked_sub(net_raised)
        .ok_or_else(|| anyhow!("net above target"))?;
    if remaining == 0 {
        return Err(anyhow!("net target reached"));
    }
    let affordable = net_from_budget(budget)?;
    let requested_net = affordable - affordable % granularity;
    if requested_net == 0 {
        return Err(anyhow!("budget below one purchase unit"));
    }
    if max_buy != 0 && requested_net > max_buy {
        return Err(anyhow!("buy above max buy"));
    }
    let net = core::cmp::min(requested_net, remaining);
    let fee = fee_on_net(net)?;
    let tokens = tokens_sold_at(net_raised + net, supply, target)?
        - tokens_sold_at(net_raised, supply, target)?;
    if tokens == 0 {
        return Err(anyhow!("zero token output"));
    }
    Ok(BuyQuote {
        requested_net,
        net,
        fee,
        tokens,
        refund: budget - net - fee,
    })
}



pub fn min_tokens_required(quote: &BuyQuote, min_tokens_out: u128) -> Result<u128> {
    if quote.net == quote.requested_net {
        return Ok(min_tokens_out);
    }
    if quote.requested_net == 0 || quote.net > quote.requested_net {
        return Err(anyhow!("invalid partial fill"));
    }


    let numerator = U256::from(min_tokens_out) * U256::from(quote.net);
    let denominator = U256::from(quote.requested_net);
    let round_up = if numerator % denominator == U256::ZERO {
        U256::ZERO
    } else {
        U256::from(1)
    };
    let rounded = numerator / denominator + round_up;
    Ok(rounded.to::<u128>())
}
