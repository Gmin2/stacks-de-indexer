#!/bin/bash
# End-to-end test using simulated block payloads.
# Mimics what stacks-core sends via POST /new_block.
#
# Usage:
#   cargo run -- dev -c examples/devnet-explorer.yaml
#   bash examples/e2e-test.sh

PORT=20445
API=4000
BASE="http://localhost:$PORT"
API_BASE="http://localhost:$API"

echo "health check..."
curl -s "$API_BASE/health"
echo ""

echo "block #1 (empty)..."
curl -s -o /dev/null -w "  %{http_code}\n" -X POST "$BASE/new_block" \
  -H "Content-Type: application/json" \
  -d '{
    "block_hash": "0xaaa111",
    "block_height": 1,
    "block_time": 1700000000,
    "burn_block_hash": "0xburn1",
    "burn_block_height": 800000,
    "index_block_hash": "0xidx001",
    "parent_block_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "parent_index_block_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "transactions": [{"txid": "0xtx_coinbase_1", "tx_index": 0, "status": "success", "raw_tx": "0x00"}],
    "events": []
  }'

echo "block #2 (stx_transfer)..."
curl -s -o /dev/null -w "  %{http_code}\n" -X POST "$BASE/new_block" \
  -H "Content-Type: application/json" \
  -d '{
    "block_hash": "0xbbb222",
    "block_height": 2,
    "block_time": 1700000030,
    "burn_block_hash": "0xburn2",
    "burn_block_height": 800001,
    "index_block_hash": "0xidx002",
    "parent_block_hash": "0xaaa111",
    "parent_index_block_hash": "0xidx001",
    "transactions": [{"txid": "0xtx_transfer_1", "tx_index": 0, "status": "success", "raw_tx": "0x00"}],
    "events": [{
      "txid": "0xtx_transfer_1", "event_index": 0, "committed": true,
      "type": "stx_transfer_event",
      "stx_transfer_event": {
        "sender": "ST1PQHQKV0RJXZFY1DGX8MNSNYVE3VGZJSRTPGZGM",
        "recipient": "ST1SJ3DTE5DN7X54YDH5D64R3BCB6A2AG2ZQ8YPD5",
        "amount": "1000000", "memo": "0x"
      }
    }]
  }'

echo "block #3 (print_event from counter)..."
curl -s -o /dev/null -w "  %{http_code}\n" -X POST "$BASE/new_block" \
  -H "Content-Type: application/json" \
  -d '{
    "block_hash": "0xccc333",
    "block_height": 3,
    "block_time": 1700000060,
    "burn_block_hash": "0xburn3",
    "burn_block_height": 800002,
    "index_block_hash": "0xidx003",
    "parent_block_hash": "0xbbb222",
    "parent_index_block_hash": "0xidx002",
    "transactions": [{"txid": "0xtx_increment_1", "tx_index": 0, "status": "success", "raw_tx": "0x00"}],
    "events": [{
      "txid": "0xtx_increment_1", "event_index": 0, "committed": true,
      "type": "contract_event",
      "contract_event": {
        "contract_identifier": "ST1PQHQKV0RJXZFY1DGX8MNSNYVE3VGZJSRTPGZGM.counter",
        "topic": "print",
        "value": {"event": "increment", "value": {"type": "uint", "value": "1"}, "caller": "ST1PQHQKV0RJXZFY1DGX8MNSNYVE3VGZJSRTPGZGM"},
        "raw_value": "0x0c00000003"
      }
    }]
  }'

echo "block #4 (two print_events)..."
curl -s -o /dev/null -w "  %{http_code}\n" -X POST "$BASE/new_block" \
  -H "Content-Type: application/json" \
  -d '{
    "block_hash": "0xddd444",
    "block_height": 4,
    "block_time": 1700000090,
    "burn_block_hash": "0xburn4",
    "burn_block_height": 800003,
    "index_block_hash": "0xidx004",
    "parent_block_hash": "0xccc333",
    "parent_index_block_hash": "0xidx003",
    "transactions": [
      {"txid": "0xtx_inc_2", "tx_index": 0, "status": "success", "raw_tx": "0x00"},
      {"txid": "0xtx_dec_1", "tx_index": 1, "status": "success", "raw_tx": "0x00"}
    ],
    "events": [
      {
        "txid": "0xtx_inc_2", "event_index": 0, "committed": true,
        "type": "contract_event",
        "contract_event": {
          "contract_identifier": "ST1PQHQKV0RJXZFY1DGX8MNSNYVE3VGZJSRTPGZGM.counter",
          "topic": "print",
          "value": {"event": "increment", "value": {"type": "uint", "value": "2"}, "caller": "ST1PQHQKV0RJXZFY1DGX8MNSNYVE3VGZJSRTPGZGM"}
        }
      },
      {
        "txid": "0xtx_dec_1", "event_index": 1, "committed": true,
        "type": "contract_event",
        "contract_event": {
          "contract_identifier": "ST1PQHQKV0RJXZFY1DGX8MNSNYVE3VGZJSRTPGZGM.counter",
          "topic": "print",
          "value": {"event": "decrement", "value": {"type": "uint", "value": "1"}, "caller": "ST1SJ3DTE5DN7X54YDH5D64R3BCB6A2AG2ZQ8YPD5"}
        }
      }
    ]
  }'

echo ""
echo "results:"
curl -s "$API_BASE/health"
echo ""
curl -s "$API_BASE/metrics" | grep -E "blocks_processed|events_matched|last_block"
echo ""
echo "graphql query:"
curl -s -X POST "$API_BASE/graphql" \
  -H "Content-Type: application/json" \
  -d '{"query": "{ contract_prints(limit: 10) { _block_height _tx_id _event_type data } }"}'
echo ""
