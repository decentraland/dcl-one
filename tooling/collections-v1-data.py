#!/usr/bin/env python3
"""Extract the collections-v1 catalogue data the shop folds Ethereum's v1 collections with.

    collections-v1-data.py wearables <collections-graph checkout> <output json>
    ETH_RPC_URL=<ethereum rpc url> collections-v1-data.py symbols <collections-v1.json> <output json>

`wearables`: the collections subgraph gives a v1 item (an `AddWearable` of a v1 collection on
Ethereum) the name, description, category, rarity and body shapes of the first static
representation whose id is the item's wearable id, searching the data arrays in the order
`getWearableV1Representation` lists them (src/modules/metadata/wearable/index.ts). This writes
those arrays, in that order, as JSON: a list of {"collection": <array name>, "wearables": [{id,
name, description, category, rarity, bodyShapes}, ...]}, each array in its file's order. The
checkout must be at the revision upstream.lock.json pins; the script prints the revision it read.
It reads the TypeScript sources as text and never runs them.

`symbols`: each v1 collection's `symbol()` (immutable, set by its constructor), read from the
endpoint in ETH_RPC_URL (never an argument: a keyed URL would show in the process list) with one
batched `eth_call` at the head's block, which it prints for upstream.lock.json, as {address: symbol}
in the order of collections-v1.json.

Both write two-space indented JSON with a trailing newline.
"""
import json
import os
import re
import subprocess
import sys
import urllib.request
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


def strip_comments(text):
    """`text` without its `//` and `/* */` comments, string literals kept whole."""
    out = []
    i = 0
    while i < len(text):
        c = text[i]
        if c in "'\"`":
            j = i + 1
            while j < len(text) and text[j] != c:
                j += 2 if text[j] == "\\" else 1
            out.append(text[i:j + 1])
            i = j + 1
        elif text.startswith("//", i):
            end = text.find("\n", i)
            i = len(text) if end < 0 else end
        elif text.startswith("/*", i):
            end = text.find("*/", i + 2)
            if end < 0:
                sys.exit("unterminated comment")
            i = end + 2
        else:
            out.append(c)
            i += 1
    return "".join(out)


def array_block(text, start):
    """The text of the `[ ... ]` array whose `[` is at text[start], strings skipped."""
    depth = 0
    i = start
    while i < len(text):
        c = text[i]
        if c in "'\"":
            _, i = string_literal(text, i)
            continue
        if c == "[":
            depth += 1
        elif c == "]":
            depth -= 1
            if depth == 0:
                return text[start:i + 1]
        i += 1
    sys.exit("unterminated array")


def wearables(path, name):
    text = strip_comments(path.read_text(encoding="utf-8"))
    export = re.search(rf"export let {name}: Wearable\[\] = (\[)", text)
    if not export:
        sys.exit(f"{path}: `export let {name}: Wearable[]` not found")
    block = array_block(text, export.start(1))
    out = []
    for match in re.finditer(r"new Wearable\(", block):
        args, _ = arguments(block, match.end() - 1)
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


def rpc_post(rpc, payload):
    """The JSON reply to one JSON-RPC request or batch."""
    request = urllib.request.Request(
        rpc,
        data=json.dumps(payload).encode(),
        headers={"content-type": "application/json", "user-agent": "collections-v1-data"},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.loads(response.read())


def abi_string(result, what):
    """The string an ABI-encoded `string` return value holds; exits on any other shape."""
    if not isinstance(result, str) or not re.fullmatch(r"0x(?:[0-9a-fA-F]{2})*", result):
        sys.exit(f"{what}: not a hex result: {result!r}")
    raw = bytes.fromhex(result[2:])
    if len(raw) < 64:
        sys.exit(f"{what}: {len(raw)} bytes, not an ABI string")
    offset = int.from_bytes(raw[0:32], "big")
    if offset + 32 > len(raw):
        sys.exit(f"{what}: string offset {offset} past the {len(raw)} bytes")
    length = int.from_bytes(raw[offset:offset + 32], "big")
    if length == 0 or offset + 32 + length > len(raw):
        sys.exit(f"{what}: string length {length} does not fit the {len(raw)} bytes")
    return raw[offset + 32:offset + 32 + length].decode("utf-8")


def eth_call_symbols(rpc, addresses):
    """(block, `symbol()` of each address), read at one block by one JSON-RPC batch."""
    head = rpc_post(rpc, {"jsonrpc": "2.0", "id": 0, "method": "eth_blockNumber", "params": []})
    if not isinstance(head, dict) or "result" not in head:
        sys.exit(f"eth_blockNumber: {head.get('error') if isinstance(head, dict) else head!r}")
    block = int(head["result"], 16)
    batch = [
        {"jsonrpc": "2.0", "id": i, "method": "eth_call", "params": [{"to": a, "data": "0x95d89b41"}, hex(block)]}
        for i, a in enumerate(addresses)
    ]
    answer = rpc_post(rpc, batch)
    if not isinstance(answer, list):
        error = answer.get("error") if isinstance(answer, dict) else answer
        sys.exit(f"the endpoint refused the batch: {error!r}")
    replies = {r.get("id"): r for r in answer if isinstance(r, dict)}
    symbols = {}
    for i, address in enumerate(addresses):
        reply = replies.get(i, {})
        if "result" not in reply:
            sys.exit(f"symbol() of {address}: {reply.get('error', 'no reply')}")
        symbols[address] = abi_string(reply["result"], f"symbol() of {address}")
    return block, symbols


def main():
    if len(sys.argv) == 4 and sys.argv[1] == "wearables":
        checkout, output = Path(sys.argv[2]), Path(sys.argv[3])
        revision = subprocess.run(
            ["git", "-C", str(checkout), "rev-parse", "HEAD"], capture_output=True, text=True, check=True
        ).stdout.strip()
        lock = json.loads((output.parent / "upstream.lock.json").read_text(encoding="utf-8"))
        pinned = [
            source["revision"]
            for source in lock["sources"]
            if source.get("repository", "").endswith("/collections-graph")
        ]
        if pinned != [revision]:
            sys.exit(f"the checkout is at {revision}, but upstream.lock.json pins {pinned}")
        order = lookup_order(strip_comments((checkout / ORDER_FILE).read_text(encoding="utf-8")))
        catalogue = [
            {"collection": name, "wearables": wearables(checkout / DATA_DIR / f"{name}.ts", name)}
            for name in order
        ]
        output.write_text(json.dumps(catalogue, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        count = sum(len(c["wearables"]) for c in catalogue)
        print(f"collections-graph {revision}: {len(catalogue)} arrays, {count} wearables -> {output}")
    elif len(sys.argv) == 4 and sys.argv[1] == "symbols":
        rpc = os.environ.get("ETH_RPC_URL", "").strip()
        if not rpc:
            sys.exit("symbols reads the endpoint from ETH_RPC_URL")
        collections, output = Path(sys.argv[2]), Path(sys.argv[3])
        addresses = [c["id"].lower() for c in json.loads(collections.read_text(encoding="utf-8"))]
        block, symbols = eth_call_symbols(rpc, addresses)
        output.write_text(json.dumps(symbols, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"{len(symbols)} symbols read at block {block} -> {output}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
