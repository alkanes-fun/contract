# Giving up pause control while keeping fee collection

The original platform credential authorizes three actions: collecting Launch fees,
collecting Pool fees, and pausing or resuming a Pool. We want to keep collecting
fees for buybacks without retaining the ability to pause trading.

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

Both transactions are confirmed. At block `970438`, the Alkanes execution trace
reports a successful Trustee `7 Deposit`. The transaction deposits one unit of
`2:98917`; `91 GetSealed` reads `2` units after it. `90 GetConfig` reads
`[2, 98917, 2, 2, 98916, 2, 102451]`, matching the two registries above.

These are state observations checked on 2026-10-08, not a substitute for accounting
for the credential's complete supply. A Bitcoin confirmation alone does not prove
that the Alkanes call succeeded, and deployment alone does not prove renunciation.

The deposit transaction is separate from the deployment transaction. `verify.py`
uses the deployment transaction, which carries the WASM in its witness.

## How fee collection works

1. The holder sends the revenue token to the Trustee and selects a listed Registry
   and project number.
2. The Trustee reads the project's Launch and Pool from that Registry. The caller
   cannot supply an arbitrary target contract.
3. The Trustee sends one unit of the platform credential to either Launch opcode
   `3` or Pool opcode `10` with `burn = 1`.
4. The target returns the credential with the fees. The Trustee checks that its
   credential balance is unchanged, keeps the credential, and returns the revenue
   token and collected assets to the caller.

Pool fees are collected as the underlying assets, rather than as LP tokens.
Collected revenue can then be used for buybacks. The Trustee does not execute or
enforce buybacks itself.

## What is removed, and what remains

The Trustee has no credential-withdrawal, pause, resume, upgrade or arbitrary-call
entry point. A deposit cannot be reversed. Once every usable unit of the platform
credential is sealed, it cannot be used outside the Trustee to pause or resume a
Pool. All affected pools must therefore be unpaused before the final deposit.

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
An external copy of the original platform credential still carries the original
permissions, regardless of the Trustee's balance.

## Verify the code and the state separately

Run the repository's usual command:

```sh
python3 verify.py
```

It rebuilds all five contracts in Docker and compares each result with both its
deployment witness and the recorded SHA-256. It verifies deployed bytes, not fee
payments, credential ownership or the completion of renunciation.

For state verification, use an Alkanes indexer at a confirmed Bitcoin height:

| Read or check | Expected meaning |
| --- | --- |
| Trustee `90 GetConfig` | `[2, 98917, count, registry_block, registry_tx, ...]`; initial list is `[2:98916, 2:102451]` |
| Trustee `91 GetSealed` | Number of platform-credential units held inside the Trustee |
| Deposit execution trace | Successful Trustee `7 Deposit`, with credential `2:98917` as incoming asset and no credential returned |
| Credential history and balances | Account for issuance and every duplication, including units held at other addresses or contracts |
| Each affected Pool `90 GetState` | Last word is `0`, meaning not paused |
| Revenue token | One unit exists at a spendable output controlled by the intended fee recipient |

The original credential can be duplicated by its holder. A positive `GetSealed`
result, or an empty balance at one wallet, is not proof that all copies have been
sealed. Check the full history and all outstanding units. This document does not
claim that such a complete holder census has been published.

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
