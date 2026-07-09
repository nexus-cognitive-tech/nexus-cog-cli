# nexus-cog-cli

Enterprise-grade command-line interface for the [Nexus Cog](https://github.com/nexus-cognitive-tech) cognitive stack.

## Subcommands (50+)

```
nexus-cog palace {rooms,summary,add-room,add-item,recall,connect}
nexus-cog brain {verify,risks,search,architecture,graph,diff,hypothesis,file}
nexus-cog cognitive {think,mirror,chain-start,chain-add,analyze}
nexus-cog causal {add-node,add-edge,forward,backward,counterfactual,pre-mortem,dump}
nexus-cog patterns {list,match,suggest}
nexus-cog provenance {record,explain,search}
nexus-cog intel {recall,store,stats,learner-stats,predict,record,suggest}
nexus-cog intent {declare,check,drift,index}
nexus-cog antifragile {adversarial,edge}
nexus-cog backup {json,sqlite}
nexus-cog decay                              # memory decay
nexus-cog repl                                # interactive REPL
nexus-cog config {show,init,add-profile}      # profile management
nexus-cog embedder info                       # embedder status
nexus-cog completions {bash,zsh,fish,...}     # shell completion
nexus-cog doctor                              # diagnostics
```

## Features

- **All 9 engines** wired up as subcommands
- **Output formats** — `--format table|json|yaml|plain`, auto-disabled colours when piped
- **Multi-profile config** — `~/.config/nexus-cog/config.toml` with named palace profiles

## Development workflow (poly-repo)

Each engine crate is an independent git repository. The CLI consumes them
through `path` dependencies in `Cargo.toml` — every sibling crate must be
checked out next to this directory:

```
$HOME/projects/
├── nexus-cog-cli/           ← this crate
├── nexus-cog-core/
├── nexus-cog-storage/
├── nexus-cog-embeddings/
├── nexus-cog-palace/
├── nexus-cog-brain/
├── nexus-cog-cognitive/
├── nexus-cog-causal/
├── nexus-cog-patterns/
├── nexus-cog-provenance/
├── nexus-cog-intel/
├── nexus-cog-intent/
└── nexus-cog-antifragile/
```

This layout is the dev convention. Engine crates therefore do **not**
build standalone (they reference sibling crates by relative path). For
a release / `cargo publish` flow, swap the `path =` lines back to
`git = "https://github.com/.../<crate>", tag = "vX.Y.Z"` and bump the tag.

> **Note on `[patch]`**: the standard cargo mechanism for redirecting git
> deps to local paths (`[patch."https://github.com/..."]`) triggers an
> upstream cargo 1.96 ambiguity error (`patch for anyhow in registry
> crates-io resolved to more than one candidate`) when the patched
> crates have transitive `crates-io` dependencies. We pin sibling deps
> via `path` directly to sidestep this until cargo upstream is fixed.
- **Shell completions** — `nexus-cog completions bash > ~/.bash_completion.d/nexus-cog`
- **REPL** — `nexus-cog repl` opens an interactive search shell
- **Env vars** — `NEXUS_COG_DB`, `NEXUS_COG_PALACE`

## Install

```sh
cargo install --git https://github.com/nexus-cognitive-tech/nexus-cog-cli
```

## Quickstart

```sh
nexus-cog config init
nexus-cog config add-profile myproject --db ./palace.db --palace myproject
nexus-cog --profile myproject palace rooms
nexus-cog --profile myproject palace recall "tokio spawn"
nexus-cog --profile myproject patterns match "async fn spawn() { ... }"
nexus-cog --profile myproject decay --format json
```

## License

Apache-2.0.


## MCP server (`nexus-cog-mcp`)

Same workspace ships an MCP server binary — every CLI subcommand is also
an MCP tool. One source of truth:

```bash
nexus-cog-mcp   # talks MCP over stdio
```

The 50+ CLI subcommands are registered as MCP tools under their snake_case
names (`palace_rooms`, `causal_forward`, `intel_predict`, etc.). The
server runs the same `nexus_cog_cli::commands::*` functions, so behaviour
and output are identical.
