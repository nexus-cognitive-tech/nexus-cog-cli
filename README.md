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
