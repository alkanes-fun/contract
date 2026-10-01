# AlkanesFUN contracts

Source of the AlkanesFUN launchpad contracts on Alkanes (Bitcoin). It is published so
that anyone can check that the contracts running on Bitcoin mainnet were built from
exactly this code.

## Deployed contracts

| Contract | Alkane ID | Deploy transaction | WASM sha256 |
| --- | --- | --- | --- |
| Launch (template) | `2:98912` | `62e4bec70574d27f2e1fad8a0db5f05efb9d1be5630c59c53f2963dce698cd22` | `a66b864aef179a6cbb7da6c89daae4489259f38f2eb3ef5f3d559e747986ca92` |
| Pool (template) | `2:98913` | `95658e8c6ca57232fed20b8f1bbde5af8f15cd65f6743fab3366211e34d1f504` | `e23e9f0abdecbfb1f9b7526ad07c1f8e39228f6b0dc45badd912e42a04e1d118` |
| Creator credential (template) | `2:98914` | `7beb74264986a7d076811f603babd7932e3e2a7241c277296b4115167e1a5f2d` | `0cef86665f7743055b5daf385905da412fe737383238d0b8399a38a82c07f0aa` |
| Registry | `2:98916` | `a9f879c9b698e1d1990206e48f14a0daf93a7eb31178880b884f5c3f2485f7d4` | `00d4336492063019e3b4f08ee2ef457bf08badc2e2e171bfb7e6c6fe739e2acf` |

The Registry is the entry point. Every project it creates gets its own Launch, Pool
and token, cloned from the three templates above. The creator credential template is
alkanes-rs's own `alkanes-std-auth-token`, deployed unchanged.

## Verify in one command

You need Docker (with BuildKit, the default since Docker 23), Python 3.8 or newer and
an internet connection.

```sh
git clone <this repository>
cd <repository>
python3 verify.py
```

The script:

1. builds every contract from this source inside Docker (`docker build --output out .`);
2. downloads each deploy transaction listed in `deployments.txt` from
   [mempool.space](https://mempool.space);
3. extracts the contract from the transaction witness and computes its sha256;
4. compares the on-chain contract, your build and `wasm.sha256`.

A successful run ends like this, with exit status 0:

```
alkanes_std_maga_registry.wasm
  tx       a9f879c9b698e1d1990206e48f14a0daf93a7eb31178880b884f5c3f2485f7d4 (block 969321)
  on chain 00d4336492063019e3b4f08ee2ef457bf08badc2e2e171bfb7e6c6fe739e2acf
  built    00d4336492063019e3b4f08ee2ef457bf08badc2e2e171bfb7e6c6fe739e2acf
  listed   00d4336492063019e3b4f08ee2ef457bf08badc2e2e171bfb7e6c6fe739e2acf
  MATCH

4 of 4 deployments match the source in this repository
```

Any difference prints `MISMATCH` or `ERROR` and exits with status 1.

Options:

| Option | Effect |
| --- | --- |
| `--skip-build` | Reuse the WASM already in `out/` |
| `--api URL` | Use another Esplora API instead of `https://mempool.space/api` |
| `--bitcoin-cli CMD` | Read the transactions from your own node, e.g. `--bitcoin-cli "bitcoin-cli -datadir=/data"` (needs `txindex=1`) |

## On x86 machines

The deployed contracts were compiled on `linux/arm64`. Cargo hashes the host platform
into symbol names, so the same source compiled on `x86_64` gives different bytes. The
`Dockerfile` therefore pins the build to `linux/arm64`:

- Docker Desktop (macOS, Windows) runs it through its built-in emulation.
- On Linux, install the emulator once:
  `docker run --privileged --rm tonistiigi/binfmt --install arm64`

An emulated build takes considerably longer than a native one.

## Verify by hand

1. **Build.** `docker build --output out .` fetches alkanes-rs at commit
   `40d3fec4746ba1940d5d77d20ab073de4524fb6b` (release `v2.2.1-rc.4`), places these crates
   in its `crates/` directory, and builds each contract with Rust 1.86.0 using the
   `Cargo.lock` in this repository. The build fails unless every WASM matches
   `wasm.sha256`. See `Dockerfile` and `build.sh`.
2. **Read the deployment.** In each deploy transaction, the witness of the input
   carries a tapscript envelope: `OP_FALSE OP_IF "BIN" <data pushes> OP_ENDIF`.
   Concatenate the data pushes after `"BIN"`, gunzip them, and take the sha256.
3. **Or ask an Alkanes node.** The `getbytecode` view of an Alkanes indexer returns the
   stored WASM for an Alkane ID; its sha256 must equal the one in the table above.

## Source

| Crate | Role |
| --- | --- |
| `common` | Shared calldata encoding, contract ABI and issuance math |
| `registry` | Creates projects: one Launch, one Pool and the project token |
| `launch` | Bonding-curve sale of a project token; graduates into its Pool |
| `pool` | Constant-product AMM for a graduated project, derived from OYL AMM |

The crate, type and file names (`alkanes-std-maga-*`, `MagaLaunch`, ...) are the
names the contracts were built under. They are compiled into the WASM, so they stay
as they are: renaming them would change the deployed bytes.

Comments were removed without moving any code: line and column positions are compiled
into the WASM, so every line stays where it was when the deployed contracts were built.

## License

Published for reading and for verifying the deployed contracts only. See [LICENSE](LICENSE).
