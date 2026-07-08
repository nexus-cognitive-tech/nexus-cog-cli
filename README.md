# nexus-cog-cli

Command-line interface for the [Nexus Cog](https://github.com/nexus-cognitive-tech) cognitive stack.

## Install

```sh
cargo install --git https://github.com/nexus-cognitive-tech/nexus-cog-cli
```

## Usage

```sh
# Specify --db and --palace BEFORE the subcommand
nexus-cog --db ./palace.db --palace default palace-rooms
nexus-cog --db ./palace.db palace-add-item --room room-0001 --key test --value "hello"
nexus-cog --db ./palace.db palace-summary
nexus-cog --db ./palace.db backup-export-json --out ./palace.json
nexus-cog --db ./palace.db decay-apply
```

## Commands

| Command | Description |
|---|---|
| `palace-rooms` | List rooms |
| `palace-summary` | Total rooms/items/connections |
| `palace-add-item` | Add an item to a room |
| `backup-export-json` | Export palace to JSON |
| `decay-apply` | Apply memory decay (half-life + TTL) |

## License

Apache-2.0.
