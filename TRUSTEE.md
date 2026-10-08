# Giving up pause control while keeping fee collection

The original platform credential authorizes three actions: collecting Launch fees,
collecting Pool fees, and pausing or resuming a Pool. We have sealed all units of
that credential, including the duplicate, in the deployed Trustee. Fee collection
for buybacks remains available; the platform can no longer use the credential to
pause or resume trading.

The Trustee separates those permissions without changing the existing contracts.
It holds the original credential and only uses it through fixed fee-collection
calls. A separate revenue token controls who receives those fees.

## Mainnet contracts and transactions

| Item | Value |
| --- | --- |
| Trustee and revenue-token ID | `2:102627` |
| Platform credential | `2:98917` |
| Initial registries, in order | `2:98916`, `2:102451` |
| Deployment block | `970419` |
| Deployment transaction | `25e9dc9207d898c53371b1c824d743ec9e37f453ff6a3890561f1a317837158d` |
| Trustee WASM SHA-256 | `ab6a07a4866bed673288b857dd5d7e4bf8e0df1acbad472951c992974009124a` |
| Credential-deposit transaction | `62626113b4b67ba0f7304b2cfc4d9190050c6f9fe838e406ad615a87156e2a93` |
| Deposit block | `970438` |

All five transactions below are confirmed and their Alkanes calls succeeded.
The sequence sealed both units of the original platform credential and exercised
both fee-collection paths through the new revenue token.

### 1. Deploy the Trustee - block 970419

[Deployment transaction](https://mempool.space/tx/25e9dc9207d898c53371b1c824d743ec9e37f453ff6a3890561f1a317837158d)

This transaction deployed Trustee `2:102627`, configured credential `2:98917` and
registries `2:98916` and `2:102451`, and minted the single revenue-token unit to
output `0`. Its initialization arguments were
`[0, 2, 98917, 2, 2, 98916, 2, 102451]`. The sealed balance started at `0`.
This is the transaction whose witness contains the WASM verified by `verify.py`.

### 2. Duplicate the credential and seal the first unit - block 970428

[First deposit transaction](https://mempool.space/tx/2b9f1cddabb591b324f4fb0cee2dedee5359f093c274dea59abd9abf10ac27f5)

This transaction spent the original credential from
`3d5bb616a773945c90f243569cd18b17f10e537623090246e1c12b875fa1e17e:0`.
It called credential `2:98917` opcode `1` to create one additional unit, then
called Trustee opcode `7 Deposit` in the same transaction. One of the two units
was sealed in the Trustee; the other remained at output `0` of this transaction.
The sealed balance became `1`.

### 3. Collect Launch fees through the Trustee - block 970432

[Launch-fee transaction](https://mempool.space/tx/c74864bebdd5df7b4482dbb775ae9a30499c49d2c87d2e23ccee0e509ce58171)

Using the revenue token from the deployment output, this transaction called
Trustee `1 ClaimLaunchFee` with Registry `2:98916`, project `3`.
The Registry resolved Launch `2:98939`; the Trustee called its opcode `3`.
The claim paid `29,440` base units of frBTC (`32:0`), or `0.00029440 frBTC`,
to output `1`, and returned the revenue token to output `0`.
The sealed credential balance remained `1`.

### 4. Collect Pool fees through the Trustee - block 970435

[Pool-fee transaction](https://mempool.space/tx/2c0bb2ee7e8164ba1c828ffd1a71c71dceba7e09b089e1d1491c31ac2bb4f081)

Reusing the revenue token returned by the previous claim, this transaction called
Trustee `2 CollectPoolFees` with Registry `2:98916`, project `1`.
The Registry resolved Pool `2:98932`; the Trustee called its opcode `10` with
`burn = 1`. The Pool collected and burned `4,967,322,297` fee LP base units,
paying `243,859` base units of frBTC (`0.00243859 frBTC`) to output `1` and
`104,746,418,679,628` base units of project token `2:98931` to output `2`.
The revenue token returned to output `0`. The sealed credential balance remained `1`.

### 5. Seal the remaining credential - block 970438

[Final deposit transaction](https://mempool.space/tx/62626113b4b67ba0f7304b2cfc4d9190050c6f9fe838e406ad615a87156e2a93)

This transaction spent the remaining credential from output `0` of the first
deposit transaction and sent that one unit to Trustee `7 Deposit`.
The Trustee retained it and returned no platform credential. The sealed balance
became `2`: the original unit and its duplicate are now both permanently held
inside the Trustee, with no remaining external unit.

The resulting credential accounting is:

| After transaction | Total credential units | Inside Trustee | Outside Trustee |
| --- | ---: | ---: | ---: |
| Deployment | 1 | 0 | 1 |
| Duplication and first deposit | 2 | 1 | 1 |
| Launch-fee claim | 2 | 1 | 1 |
| Pool-fee claim | 2 | 1 | 1 |
| Final deposit | 2 | 2 | 0 |

At block `970438`, `91 GetSealed` returns `[2]` and `90 GetConfig` returns
`[2, 98917, 2, 2, 98916, 2, 102451]`. The final deposit does not consume the
revenue token, so the fee-collection mechanism remains available with both
credential units sealed.

## How fee collection works

1. The holder sends the revenue token to the Trustee and selects a listed Registry
   and project number.
2. The Trustee reads the project's Launch and Pool from that Registry. The caller
   cannot supply an arbitrary target contract.
3. The Trustee sends one unit of the platform credential to Launch opcode `3`,
   or to Pool opcode `10` with `burn = 1`.
4. The target returns the credential with the fees. The Trustee checks that its
   credential balance is unchanged, keeps the credential, and returns the revenue
   token and collected assets to the caller.

Pool fees are collected as the underlying assets, rather than as LP tokens.
Collected revenue can then be used for buybacks. The Trustee does not execute or
enforce buybacks itself.

## What is removed, and what remains

The Trustee has no credential-withdrawal, pause, resume, upgrade or arbitrary-call
entry point. Deposits cannot be reversed. Both credential units are now sealed,
so the original credential cannot be used outside the Trustee to pause or resume
a Pool. Sealing the credential does not itself change a Pool's existing pause state.

The revenue token is one unit of the Trustee's own ID, minted once during
initialization. It is returned after each claim and cannot be minted again through
the Trustee. Holding it permits fee collection, but it does not satisfy the Pool's
platform-credential check.

There is one other retained capability: opcode `20` lets the revenue-token holder
create a Registry by cloning the fixed master Registry. Only the raise target,
purchase unit and default sale duration can change. Templates, quote asset, supply
and fee shares are copied from the master. This is not an upgrade to an existing
Registry, Launch or Pool. Future V2 contracts are a separate deployment.

Existing registries cannot be added or removed after initialization. New entries
can only be created by the Trustee's own cloning operation. Correct initial
Registry and template code is part of the trust boundary: matching configuration
values alone do not prove that an arbitrary contract runs the expected code.

The revenue token remains a valuable credential. Losing it prevents future claims
and Registry creation through the Trustee; transferring it transfers those rights.

## Verify the code and the state separately

Run the repository's usual command:

```sh
python3 verify.py
```

It rebuilds all five contracts in Docker and compares each result with both its
deployment witness and the recorded SHA-256. The transaction sequence above
records the deposits and fee payments separately from that reproducible code check.

For state verification, use an Alkanes indexer at a confirmed Bitcoin height:

| Read or check | Expected meaning |
| --- | --- |
| Trustee `90 GetConfig` | `[2, 98917, count, registry_block, registry_tx, ...]`; initial list is `[2:98916, 2:102451]` |
| Trustee `91 GetSealed` | Number of platform-credential units held inside the Trustee |
| Deposit execution trace | Successful Trustee `7 Deposit`, with credential `2:98917` as incoming asset and no credential returned |
| Credential history and balances | Account for issuance and every duplication, including units held at other addresses or contracts |
| Each affected Pool `90 GetState` | Last word is `0`, meaning not paused |
| Revenue token | One unit exists at a spendable output controlled by the intended fee recipient |

Credential opcode `1` requires possession of the original credential. Both units
are now held by the Trustee, which has no path to that opcode and no withdrawal
entry point. The revenue token cannot authorize credential duplication.

## Trustee entry points

| Opcode | Purpose |
| --- | --- |
| `0` | Initialize once and mint the single revenue-token unit |
| `1` | Claim a listed project's platform Launch fees; requires the revenue token |
| `2` | Collect a listed project's Pool fees with `burn = 1`; requires the revenue token |
| `7` | Deposit only the platform credential, permanently |
| `20` | Clone the master Registry with three configurable values; requires the revenue token |
| `90` | Read the credential and Registry list |
| `91` | Read the sealed credential balance |
| `99` | Read the contract name |

These numbers belong to the Trustee. For example, Trustee opcode `1` calls Launch
opcode `3`; Trustee opcode `2` calls Pool opcode `10`. There is no Trustee path to
Pool opcode `4 SetPaused`.
