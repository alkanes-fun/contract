#!/usr/bin/env python3
"""Verify that the deployed contracts are the ones built from this repository.

    python3 verify.py                      rebuild with Docker, then check every deployment
    python3 verify.py --skip-build         check against the WASM already in out/
    python3 verify.py --api URL            any Esplora API (default: https://mempool.space/api)
    python3 verify.py --bitcoin-cli CMD    read transactions from your own node instead,
                                           e.g. --bitcoin-cli "bitcoin-cli -datadir=/data"

Each transaction listed in deployments.txt carries a contract in the witness of its
reveal input (an envelope tagged "BIN", gzip-compressed). The script extracts it,
hashes it, and compares it with the local build and with wasm.sha256.
"""
import argparse
import gzip
import hashlib
import json
import shlex
import subprocess
import sys
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"


def read_list(path):
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split()
        if len(parts) == 2:
            rows.append((parts[0], parts[1]))
    return rows


def fetch_hex(txid, api, cli):
    if cli:
        cmd = shlex.split(cli) + ["getrawtransaction", txid]
        return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout.strip()
    req = urllib.request.Request(f"{api.rstrip('/')}/tx/{txid}/hex", headers={"User-Agent": "verify"})
    with urllib.request.urlopen(req, timeout=60) as resp:
        return resp.read().decode().strip()


def fetch_status(txid, api, cli):
    try:
        if cli:
            cmd = shlex.split(cli) + ["getrawtransaction", txid, "1"]
            tx = json.loads(subprocess.run(cmd, check=True, capture_output=True, text=True).stdout)
            return f"{tx.get('confirmations', 0)} confirmations" if tx.get("confirmations") else "unconfirmed"
        req = urllib.request.Request(f"{api.rstrip('/')}/tx/{txid}/status", headers={"User-Agent": "verify"})
        with urllib.request.urlopen(req, timeout=60) as resp:
            status = json.loads(resp.read())
        return f"block {status['block_height']}" if status.get("confirmed") else "unconfirmed"
    except Exception as exc:
        return f"status unknown ({exc})"


class Reader:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def take(self, n):
        chunk = self.data[self.pos:self.pos + n]
        if len(chunk) != n:
            raise ValueError("truncated transaction")
        self.pos += n
        return chunk

    def varint(self):
        first = self.take(1)[0]
        if first < 0xfd:
            return first
        return int.from_bytes(self.take({0xfd: 2, 0xfe: 4, 0xff: 8}[first]), "little")


def witnesses(raw):
    r = Reader(raw)
    r.take(4)
    if raw[4:6] != b"\x00\x01":
        return []
    r.take(2)
    inputs = r.varint()
    for _ in range(inputs):
        r.take(36)
        r.take(r.varint())
        r.take(4)
    for _ in range(r.varint()):
        r.take(8)
        r.take(r.varint())
    stacks = []
    for _ in range(inputs):
        stacks.append([r.take(r.varint()) for _ in range(r.varint())])
    return stacks


def pushes(script):
    i, items = 0, []
    while i < len(script):
        op = script[i]
        i += 1
        if 1 <= op <= 75:
            n = op
        elif op in (76, 77, 78):
            width = {76: 1, 77: 2, 78: 4}[op]
            n = int.from_bytes(script[i:i + width], "little")
            i += width
        else:
            items.append(op)
            continue
        items.append(script[i:i + n])
        i += n
    return items


def envelope(raw):
    for stack in witnesses(raw):
        if stack and stack[-1][:1] == b"\x50":
            stack = stack[:-1]
        if len(stack) < 2:
            continue
        items = pushes(stack[-2])
        for k, item in enumerate(items):
            if item == b"BIN":
                body = bytearray()
                for part in items[k + 1:]:
                    if part == 0x68:
                        break
                    if isinstance(part, bytes):
                        body += part
                return gzip.decompress(bytes(body))
    raise ValueError("no BIN envelope in any input witness")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--api", default="https://mempool.space/api")
    ap.add_argument("--bitcoin-cli", metavar="CMD")
    a = ap.parse_args()

    if not a.skip_build:
        print("building with Docker (docker build --output out .)", flush=True)
        subprocess.run(["docker", "build", "--output", str(OUT), str(HERE)], check=True)

    expected = {name: digest for digest, name in read_list(HERE / "wasm.sha256")}
    deployments = read_list(HERE / "deployments.txt")
    if not deployments:
        sys.exit("deployments.txt lists no transactions")

    failed = 0
    for name, txid in deployments:
        print(f"\n{name}\n  tx       {txid} ({fetch_status(txid, a.api, a.bitcoin_cli)})")
        try:
            onchain = hashlib.sha256(envelope(bytes.fromhex(fetch_hex(txid, a.api, a.bitcoin_cli)))).hexdigest()
        except Exception as exc:
            print(f"  on chain ERROR {exc}")
            failed += 1
            continue
        local = OUT / name
        built = hashlib.sha256(local.read_bytes()).hexdigest() if local.exists() else None
        print(f"  on chain {onchain}")
        print(f"  built    {built or 'missing: run without --skip-build'}")
        print(f"  listed   {expected.get(name, 'not listed in wasm.sha256')}")
        ok = onchain == built == expected.get(name)
        print(f"  {'MATCH' if ok else 'MISMATCH'}")
        failed += not ok

    print(f"\n{len(deployments) - failed} of {len(deployments)} deployments match the source in this repository")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
