# `dk mcp serve`

Expose `dk` over the Model Context Protocol so an MCP-capable agent can call
review as a tool.

## Synopsis

```
dk mcp serve [--transport http|stdio] [--host <H>] [--port <P>] [--path <PATH>]
```

| Flag | Default | Meaning |
|------|---------|---------|
| `--transport` | `http` | `http` (Streamable HTTP) or `stdio` (stdin/stdout JSON-RPC). |
| `--host` | `127.0.0.1` | Bind address (http only). |
| `--port` | `8080` | Bind port (http only). |
| `--path` | `/mcp` | HTTP path prefix (http only). |

`--host/--port/--path` are rejected with `stdio`.

## What's exposed

The server exposes exactly **one tool, `dk.review`**. The other commands
(`init`, `doctor`, `check`) are intentionally not exposed as tools.

## Verify the tool list (stdio)

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"x","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | timeout 8 dk mcp serve --transport stdio
```

The `tools/list` response contains `"name":"dk.review"`.

## HTTP

```sh
dk mcp serve --transport http --host 127.0.0.1 --port 8080 --path /mcp
# MCP endpoint: http://127.0.0.1:8080/mcp
```

## Registering with an agent

`dk` doesn't include an `mcp install` command. Register it manually in your
agent's MCP config — e.g. an `http` server at `http://127.0.0.1:8080/mcp`, or a
`stdio` server whose command is `dk mcp serve --transport stdio`.

## Gotchas

- Progress lines from a triggered `dk.review` go to **stderr**, so they don't
  corrupt the stdio JSON-RPC stream on stdout.
- The exposed `review` tool still shells out to the configured agent — the MCP
  server host needs that agent on `PATH`.
