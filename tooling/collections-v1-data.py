#!/usr/bin/env python3
"""Extract the collections-v1 catalogue data the shop folds Ethereum's v1 collections with.

    collections-v1-data.py wearables <collections-graph checkout> <output json>
    collections-v1-data.py symbols <ethereum rpc url> <collections-v1.json> <output json>

`wearables`: the collections subgraph gives a v1 item (an `AddWearable` of a v1 collection on
Ethereum) the name, description, category, rarity and body shapes of the first static
representation whose id is the item's wearable id, searching the data arrays in the order
`getWearableV1Representation` lists them (src/modules/metadata/wearable/index.ts). This writes
those arrays, in that order, as JSON: a list of {"collection": <array name>, "wearables": [{id,
name, description, category, rarity, bodyShapes}, ...]}, each array in its file's order. The
checkout must be at the revision upstream.lock.json pins; the script prints the revision it read.
It reads the TypeScript sources as text and never runs them.

`symbols`: each v1 collection's `symbol()` (immutable, set by its constructor), read with one
batched `eth_call` at the latest block, as {address: symbol} in the order of collections-v1.json.

Both write two-space indented JSON with a trailing newline.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

ORDER_FILE = "src/modules/metadata/wearable/index.ts"
DATA_DIR = "src/data/wearablesV1"


def lookup_order(source):
    """The array names `getWearableV1Representation` searches, in order."""
    body = re.search(
        r"export function getWearableV1Representation\(.*?let allCollections: WearableRepresentation\[\]\[\] = \[(.*?)\]",
        source,
        re.S,
    )
    if not body:
        sys.exit(f"{ORDER_FILE}: getWearableV1Representation's list not found")
    names = [n.strip() for n in body.group(1).split(",") if n.strip()]
    if not names or any(not re.fullmatch(r"[a-z0-9_]+", n) for n in names):
        sys.exit(f"{ORDER_FILE}: unexpected entries in the list: {names}")
    return names


def string_literal(text, i):
    """The JS string literal starting at text[i] (a quote) and the index after it."""
    quote = text[i]
    out = []
    i += 1
    while True:
        c = text[i]
        if c == "\\":
            nxt = text[i + 1]
            escapes = {"n": "\n", "t": "\t", "\\": "\\", "'": "'", '"': '"'}
            if nxt not in escapes:
                sys.exit(f"unsupported escape \\{nxt}")
            out.append(escapes[nxt])
            i += 2
        elif c == quote:
            return "".join(out), i + 1
        elif c == "\n":
            sys.exit("unterminated string literal")
        else:
            out.append(c)
            i += 1


def arguments(text, i):
    """The arguments of a `new Wearable(` call whose '(' is at text[i]: strings and string
    arrays only; the index after the closing ')'."""
    args = []
    i += 1
    while True:
        while text[i] in " \t\r\n,":
            i += 1
        c = text[i]
        if c == ")":
            return args, i + 1
        if c in "'\"":
            value, i = string_literal(text, i)
            args.append(value)
        elif c == "[":
            items = []
            i += 1
            while True:
                while text[i] in " \t\r\n,":
                    i += 1
                if text[i] == "]":
                    i += 1
                    break
                if text[i] not in "'\"":
                    sys.exit(f"unexpected array element at {text[i:i + 20]!r}")
                value, i = string_literal(text, i)
                items.append(value)
            args.append(items)
        else:
            sys.exit(f"unexpected argument at {text[i:i + 20]!r}")


def wearables(path, name):
    text = path.read_text(encoding="utf-8")
    if not re.search(rf"export let {name}: Wearable\[\] = \[", text):
        sys.exit(f"{path}: `export let {name}: Wearable[]` not found")
    out = []
    for match in re.finditer(r"new Wearable\(", text):
        args, _ = arguments(text, match.end() - 1)
        if len(args) != 6 or not all(isinstance(a, str) for a in args[:5]) or not isinstance(args[5], list):
            sys.exit(f"{path}: a Wearable with unexpected arguments: {args}")
        wid, wname, description, category, rarity, body_shapes = args
        out.append({
            "id": wid,
            "name": wname,
            "description": description,
            "category": category,
            "rarity": rarity,
            "bodyShapes": body_shapes,
        })
    if not out:
        sys.exit(f"{path}: no wearables")
    return out


def eth_call_symbols(rpc, addresses):
    """`symbol()` of each address, by one JSON-RPC batch."""
    batch = [
        {"jsonrpc": "2.0", "id": i, "method": "eth_call", "params": [{"to": a, "data": "0x95d89b41"}, "latest"]}
        for i, a in enumerate(addresses)
    ]
    answer = subprocess.run(
        ["curl", "-s", "-m", "60", rpc, "-H", "content-type: application/json", "--data", json.dumps(batch)],
        capture_output=True, text=True, check=True,
    ).stdout
    replies = {r["id"]: r for r in json.loads(answer)}
    symbols = {}
    for i, address in enumerate(addresses):
        reply = replies.get(i, {})
        if "result" not in reply:
            sys.exit(f"symbol() of {address}: {reply.get('error')}")
        raw = bytes.fromhex(reply["result"][2:])
        offset = int.from_bytes(raw[0:32], "big")
        length = int.from_bytes(raw[offset:offset + 32], "big")
        symbols[address] = raw[offset + 32:offset + 32 + length].decode("utf-8")
    return symbols


def main():
    if len(sys.argv) == 4 and sys.argv[1] == "wearables":
        checkout, output = Path(sys.argv[2]), Path(sys.argv[3])
        revision = subprocess.run(
            ["git", "-C", str(checkout), "rev-parse", "HEAD"], capture_output=True, text=True, check=True
        ).stdout.strip()
        order = lookup_order((checkout / ORDER_FILE).read_text(encoding="utf-8"))
        catalogue = [
            {"collection": name, "wearables": wearables(checkout / DATA_DIR / f"{name}.ts", name)}
            for name in order
        ]
        output.write_text(json.dumps(catalogue, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        count = sum(len(c["wearables"]) for c in catalogue)
        print(f"collections-graph {revision}: {len(catalogue)} arrays, {count} wearables -> {output}")
    elif len(sys.argv) == 5 and sys.argv[1] == "symbols":
        rpc, collections, output = sys.argv[2], Path(sys.argv[3]), Path(sys.argv[4])
        addresses = [c["id"].lower() for c in json.loads(collections.read_text(encoding="utf-8"))]
        symbols = eth_call_symbols(rpc, addresses)
        output.write_text(json.dumps(symbols, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"{len(symbols)} symbols -> {output}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
