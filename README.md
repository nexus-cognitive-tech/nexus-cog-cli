# nexus-cog-cli

Enterprise-grade command-line interface for the
[Nexus Cog](https://github.com/nexus-cognitive-tech) cognitive stack.
Backed by [`nexus-cog-neural`](../nexus-cog-neural/), a single
brain-like cortex that wires together thalamus, cortical hierarchy,
hippocampus, amygdala, basal ganglia, attention, working memory,
neuromodulators, global workspace, replay buffer and sleep cycle.

## Subcommands

```
nexus-cog palace {rooms,summary,add-room,add-item,recall,connect}
nexus-cog brain {verify,risks,search,architecture,graph,diff,hypothesis,file}
nexus-cog cognitive {think,mirror,chain-start,chain-add,analyze}
nexus-cog causal {add-node,add-edge,forward,backward,counterfactual,pre-mortem,blast,dump}
nexus-cog patterns {list,match,suggest}
nexus-cog provenance {record,explain,search}
nexus-cog intel {recall,store,stats,learner-stats,predict,record,suggest}
nexus-cog intent {declare,check,drift,index}
nexus-cog antifragile {adversarial,edge}
nexus-cog backup {json,sqlite}
nexus-cog decay                              # cortex sleep cycle
nexus-cog repl                                # interactive REPL
nexus-cog config {show,init,add-profile}      # profile management
nexus-cog embedder info                       # embedder status
nexus-cog completions {bash,zsh,fish,...}     # shell completion
nexus-cog doctor                              # diagnostics
```

## Features

- **One brain** — every brain-related subcommand (`palace`, `brain`,
  `cognitive`, `intel`, `intent`, `decay`) routes through the cortex in
  `nexus-cog-neural`. No duplicated engines, no shims.
- **Per-workspace DB** — the SQLite database lives at
  `<workspace>/.nexus-cog/palace.db`. Override with `--db` or
  `--workspace`.
- **Output formats** — `--format table|json|yaml|plain`, auto-disabled
  colours when piped.
- **Multi-profile config** — `~/.config/nexus-cog/config.toml` with
  named profiles.
- **Shell completions** — `nexus-cog completions bash > ~/.bash_completion.d/nexus-cog`.
- **REPL** — `nexus-cog repl` opens an interactive recall shell.
- **Env vars** — `NEXUS_COG_DB`, `NEXUS_COG_WORKSPACE`.

## Poly-repo layout

Each engine crate is an independent git repository. The CLI consumes
them through `path` dependencies in `Cargo.toml` — every sibling
crate must be checked out next to this directory:

```
$HOME/projects/
├── nexus-cog-cli/           ← this crate
├── nexus-cog-core/
├── nexus-cog-storage/
├── nexus-cog-embeddings/
├── nexus-cog-neural/         ← the brain
├── nexus-cog-causal/
├── nexus-cog-patterns/
├── nexus-cog-provenance/
└── nexus-cog-antifragile/
```

This layout is the dev convention. Engine crates therefore do **not**
build standalone (they reference sibling crates by relative path). For
a release / `cargo publish` flow, swap the `path =` lines back to
`git = "https://github.com/.../<crate>", tag = "vX.Y.Z"` and bump the
tag.

> **Note on `[patch]`**: the standard cargo mechanism for redirecting
> git deps to local paths (`[patch."https://github.com/..."]`) triggers
> an upstream cargo 1.96 ambiguity error
> (`patch for <transitive dep> in registry crates-io resolved to more
> than one candidate`) when the patched crates have transitive
> `crates-io` dependencies. We pin sibling deps via `path` directly to
> sidestep this until cargo upstream is fixed.

## Install

```sh
cargo install --git https://github.com/nexus-cognitive-tech/nexus-cog-cli
```

## Quickstart

```sh
nexus-cog --workspace ./myproject palace summary
nexus-cog --workspace ./myproject intent declare auth "JWT bearer auth"
nexus-cog --workspace ./myproject intent check auth --current-code 'fn verify(u: &str, p: &str) -> bool { u == "admin" && p == "hunter2" }'
nexus-cog --workspace ./myproject causal blast core_entity
nexus-cog --workspace ./myproject provenance record src/main.rs author "first commit" model_output "Initial scaffold" --prompt "scaffold"
nexus-cog decay                  # one cortex sleep cycle
nexus-cog repl                   # interactive hippocampal recall
```

## MCP server

Every CLI subcommand is also an MCP tool. Run `nexus-cog mcp` to expose
the full toolset over stdio.

```bash
nexus-cog mcp    # talks MCP over stdio
```

The 30+ tools are registered under their snake_case names
(`palace_recall`, `causal_blast`, `intent_check`, `provenance_record`,
…). One source of truth — the CLI and the MCP server share the same
`nexus_cog_cli::commands::*` functions, so behaviour and output are
identical.

## License

Apache-2.0.
