# Built-in commands: `spec`, `completion`, `version`

These are provided by `cli-framework`'s `AppBuilder::build`, not defined in
`dk`. They appear automatically in `dk --help`.

## `dk spec`

Export the CLI command surface (the registered commands, args, metadata) as a
machine-readable document — useful for tooling, docs, or LLM command resolution.

```
dk spec [--format json|yaml|markdown] [--output <path>] [--include-hidden]
```

| Flag | Default | Meaning |
|------|---------|---------|
| `--format` | `json` | `json`, `yaml`, or `markdown`. |
| `--output <path>` | stdout | Write to a file instead. |
| `--include-hidden` | off | Include commands marked `hidden: true`. |

```sh
dk spec --format json | jq '.commands[].id'
dk spec --format markdown --output COMMANDS.md
```

The JSON has `schemaVersion: "cli-framework.command-surface.v1"`, an `app`
block (`name`, `version`), and a `commands` array (each with `path`, `id`,
`summary`, `syntax`, `category`, `args`, …).

## `dk completion`

Emit a shell completion stub for the top-level subcommands.

```
dk completion <shell>
```

`<shell>` is one of `bash`, `zsh`, `fish`, `powershell`, `pwsh`.

```sh
dk completion bash > /etc/bash_completion.d/dk     # or source it
dk completion zsh  > "${fpath[1]}/_dk"
```

The bash stub defines `_dk()` completing `check completion doctor init mcp
review spec` and registers `complete -F _dk dk`.

## `dk version`

```sh
dk version        # -> "dk 0.1.0"
dk --version      # same
```

Version comes from `AppBuilder::with_version("dk", env!("CARGO_PKG_VERSION"))`.

## `dk help`

`dk help` / `dk <cmd> --help` print usage from the registered `CommandSpec`s.
