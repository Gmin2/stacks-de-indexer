# stacks-indexer CLI

Command-line interface for the Stacks indexer.

## Installation

```
cargo install stacks-indexer
```

Or build from source:

```
cargo build --release
```

The binary is at `target/release/stacks-indexer`.

## Commands

### `dev`

Start in development mode with verbose logging and GraphQL playground.

```
stacks-indexer dev -c stacks-indexer.yaml
```

### `start`

Start in production mode with info-level logging.

```
stacks-indexer start -c stacks-indexer.yaml
```

### `status`

Show sync status: last block height, database path, and table row counts.

```
stacks-indexer status -c stacks-indexer.yaml
```

Output:

```
Indexer Status
  Database:          ./data/indexer.db
  Last block height: 158392
  Last block hash:   0x1a2b3c...
  Table counter_events: 42 rows
  Table stx_transfers: 1230 rows
```

### `reset`

Delete the database and start fresh.

```
stacks-indexer reset -c stacks-indexer.yaml
```

### `init`

Scaffold a new indexer project with a template config file.

```
stacks-indexer init my-indexer
```

Creates `my-indexer/stacks-indexer.yaml` with a starter config.

### `codegen`

Generate Rust type definitions from the YAML config.

```
stacks-indexer codegen -c stacks-indexer.yaml -o src/generated.rs
```

## Options

All commands that take a config file use `-c` / `--config` with a default of `stacks-indexer.yaml` in the current directory.

## Environment variables

| Variable | Description |
|---|---|
| `RUST_LOG` | Override log level (e.g. `debug`, `stacks_indexer=trace`) |

## Examples

Run any of the included examples:

```
cargo run --example decode_clarity_value
cargo run --example parse_block_payload
cargo run --example local_devnet
```

See the `examples/` directory for more, including an e2e test script and a testnet block replay tool.
