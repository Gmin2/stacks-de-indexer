#!/usr/bin/env python3
"""
Replay real testnet blocks from Hiro API into the local indexer.

Fetches blocks + transactions + events from api.testnet.hiro.so,
transforms them from Hiro API format into stacks-core event observer
format, and POSTs them to the local indexer.

Usage:
    # Start the indexer first:
    cargo run -- dev -c examples/testnet-replay/indexer.yaml

    # Then replay blocks:
    python3 examples/testnet-replay/replay.py 100 5     # blocks 100-104
    python3 examples/testnet-replay/replay.py            # defaults: start=1, count=10
"""

import json
import sys
import time
import urllib.error
import urllib.request

API = "https://api.testnet.hiro.so"
INDEXER = "http://127.0.0.1:30445"

start = int(sys.argv[1]) if len(sys.argv) > 1 else 1
count = int(sys.argv[2]) if len(sys.argv) > 2 else 10


def fetch_json(url):
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "stacks-indexer/0.1"})
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        if e.code == 429:
            print("  rate limited, waiting 25s...")
            time.sleep(25)
            return fetch_json(url)
        body = e.read().decode()[:200] if hasattr(e, "read") else ""
        print(f"  fetch error: {e} {body}")
        return None
    except Exception as e:
        print(f"  fetch error: {e}")
        return None


def transform_event(ev, tx_status):
    txid = ev.get("tx_id", "")
    idx = ev.get("event_index", 0)
    committed = tx_status == "success"
    etype = ev.get("event_type", "")

    base = {"txid": txid, "event_index": idx, "committed": committed}

    if etype == "stx_asset":
        asset = ev.get("asset", {})
        sub = asset.get("asset_event_type", "transfer")
        mapping = {
            "transfer": ("stx_transfer_event", ["sender", "recipient", "amount", "memo"]),
            "mint": ("stx_mint_event", ["recipient", "amount"]),
            "burn": ("stx_burn_event", ["sender", "amount"]),
        }
        if sub not in mapping:
            return None
        key, fields = mapping[sub]
        base["type"] = key
        base[key] = {f: asset.get(f, "0x" if f == "memo" else "") for f in fields}

    elif etype == "stx_lock":
        lock = ev.get("stx_lock_event", {})
        base["type"] = "stx_lock_event"
        base["stx_lock_event"] = {
            "locked_amount": lock.get("locked_amount", "0"),
            "unlock_height": lock.get("unlock_height", 0),
            "locked_address": lock.get("locked_address", ""),
            "contract_identifier": "",
        }

    elif etype == "smart_contract_log":
        log = ev.get("contract_log", {})
        base["type"] = "contract_event"
        base["contract_event"] = {
            "contract_identifier": log.get("contract_id", ""),
            "topic": log.get("topic", "print"),
            "value": log.get("value", {}),
            "raw_value": log.get("value", {}).get("hex", ""),
        }

    elif etype == "fungible_token_asset":
        asset = ev.get("asset", {})
        sub = asset.get("asset_event_type", "transfer")
        mapping = {
            "transfer": ("ft_transfer_event", ["asset_id:asset_identifier", "sender", "recipient", "amount"]),
            "mint": ("ft_mint_event", ["asset_id:asset_identifier", "recipient", "amount"]),
            "burn": ("ft_burn_event", ["asset_id:asset_identifier", "sender", "amount"]),
        }
        if sub not in mapping:
            return None
        key, fields = mapping[sub]
        base["type"] = key
        base[key] = {}
        for f in fields:
            src, dst = f.split(":") if ":" in f else (f, f)
            base[key][dst] = asset.get(src, "")

    elif etype == "non_fungible_token_asset":
        asset = ev.get("asset", {})
        sub = asset.get("asset_event_type", "transfer")
        mapping = {
            "transfer": "nft_transfer_event",
            "mint": "nft_mint_event",
            "burn": "nft_burn_event",
        }
        if sub not in mapping:
            return None
        key = mapping[sub]
        base["type"] = key
        data = {"asset_identifier": asset.get("asset_id", "")}
        if sub == "transfer":
            data["sender"] = asset.get("sender", "")
            data["recipient"] = asset.get("recipient", "")
        elif sub == "mint":
            data["recipient"] = asset.get("recipient", "")
        elif sub == "burn":
            data["sender"] = asset.get("sender", "")
        data["value"] = asset.get("value", {})
        data["raw_value"] = asset.get("value", {}).get("hex", "")
        base[key] = data
    else:
        return None

    return base


def replay_block(height):
    print(f"block #{height}", end="")

    block = fetch_json(f"{API}/extended/v2/blocks/{height}")
    if not block:
        print(" - not found, skipping")
        return False

    block_hash = block["hash"]
    txs_data = fetch_json(f"{API}/extended/v2/blocks/{block_hash}/transactions?limit=50")
    if not txs_data:
        print(" - no transactions, skipping")
        return False

    transactions = []
    all_events = []

    for tx in txs_data.get("results", []):
        transactions.append({
            "txid": tx["tx_id"],
            "tx_index": tx.get("tx_index", 0),
            "status": "success" if tx.get("tx_status") == "success" else "abort_by_response",
            "raw_tx": "0x00",
            "raw_result": tx.get("tx_result", {}).get("hex", "0x0703"),
        })

        if tx.get("event_count", 0) == 0:
            continue

        ev_data = fetch_json(f"{API}/extended/v1/tx/events?tx_id={tx['tx_id']}&limit=100")
        if not ev_data:
            continue

        for ev in ev_data.get("events", []):
            core_ev = transform_event(ev, tx.get("tx_status", ""))
            if core_ev:
                all_events.append(core_ev)

    payload = {
        "block_hash": block_hash,
        "block_height": height,
        "block_time": block.get("block_time", 0),
        "burn_block_hash": block["burn_block_hash"],
        "burn_block_height": block["burn_block_height"],
        "index_block_hash": block["index_block_hash"],
        "parent_block_hash": block["parent_block_hash"],
        "parent_index_block_hash": block["parent_index_block_hash"],
        "transactions": transactions,
        "events": all_events,
        "matured_miner_rewards": [],
        "signer_signature": [],
    }

    print(f" - {len(transactions)} txs, {len(all_events)} events", end="")

    try:
        data = json.dumps(payload).encode()
        req = urllib.request.Request(
            f"{INDEXER}/new_block", data=data,
            headers={"Content-Type": "application/json"}, method="POST",
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            print(f" -> {resp.status}")
    except Exception as e:
        print(f" -> FAILED: {e}")
        return False

    return True


print(f"replaying blocks {start}..{start + count - 1}\n")

success = 0
for i, h in enumerate(range(start, start + count)):
    if i > 0:
        time.sleep(2)
    if replay_block(h):
        success += 1

print(f"\n{success}/{count} blocks replayed")

health = fetch_json("http://127.0.0.1:4000/health")
if health:
    print(f"indexer: {json.dumps(health)}")

try:
    with urllib.request.urlopen("http://127.0.0.1:4000/metrics", timeout=5) as resp:
        for line in resp.read().decode().split("\n"):
            if line and not line.startswith("#") and any(
                k in line for k in ["blocks_processed", "events_matched", "last_block_height"]
            ):
                print(f"  {line}")
except Exception:
    pass
