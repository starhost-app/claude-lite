---
name: container-guide
description: How to work efficiently inside this container -- prefer batch MCP tools over one-off Bash calls, and use pacman to install missing tools.
---

# Working in this container

This container ships a custom MCP server, `batch-tools`, with four tools:

- `run_commands` -- run many shell commands in one call, sequentially or in parallel
- `write_files` -- write many files in one call, creating parent directories automatically
- `make_dirs` -- create many directories in one call
- `write_tree` -- create an entire file/directory tree from a nested JSON object in one call

**Always prefer these batch tools over calling Bash, Write, or a file-editing tool once per file/command/directory.** Each tool call has overhead and consumes context -- batching saves round trips. Use:

- `run_commands` instead of chaining multiple separate Bash calls
- `write_tree` when scaffolding a new project or many files/dirs at once
- `write_files` / `make_dirs` when you already know the exact list of files/dirs to create

Only fall back to a single plain Bash call for one-off commands that don't fit these shapes.

## Installing tools

This is an Arch Linux container. If a tool you need isn't installed:

```
pacman -Sy --noconfirm <package>
```

Search for a package name first if unsure:

```
pacman -Ss <term>
```

Prefer running these through `run_commands` rather than a plain Bash call when installing more than one package.
