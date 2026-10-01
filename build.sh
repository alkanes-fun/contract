#!/bin/sh
set -eu
REPO=/home/satoshi/repo
SRC=/home/satoshi/src
OUT=/home/satoshi/out
mkdir -p "$SRC" "$OUT"
curl -sSfL https://codeload.github.com/kungfuflex/alkanes-rs/tar.gz/40d3fec4746ba1940d5d77d20ab073de4524fb6b | tar -xz -C "$SRC" --strip-components=1
cp -r "$REPO/common" "$SRC/crates/common"
for c in launch registry pool; do
  d="$SRC/crates/alkanes-std-maga-$c"
  cp -r "$REPO/$c" "$d"
  mkdir -p "$d/.cargo"
  cp "$SRC/crates/alkanes-std-auth-token/.cargo/config.toml" "$d/.cargo/config.toml"
done
sed -i 's|^members = \[$|members = [\n    "crates/common",\n    "crates/alkanes-std-maga-launch",\n    "crates/alkanes-std-maga-registry",\n    "crates/alkanes-std-maga-pool",|' "$SRC/Cargo.toml"
cp "$REPO/Cargo.lock" "$SRC/Cargo.lock"
for d in alkanes-std-auth-token alkanes-std-maga-launch alkanes-std-maga-registry alkanes-std-maga-pool; do
  (cd "$SRC/crates/$d" && CARGO_TARGET_DIR="$SRC/target/alkanes" cargo build --release --locked)
  cp "$SRC/target/alkanes/wasm32-unknown-unknown/release/$(echo "$d" | tr - _).wasm" "$OUT/"
done
cd "$OUT"
sha256sum *.wasm
sha256sum -c "$REPO/wasm.sha256"
