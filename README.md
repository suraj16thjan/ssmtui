# ssmtui

A terminal UI for AWS Systems Manager Parameter Store built with `ratatui` and the AWS Rust SDK.

`ssmtui` lets you browse, filter, view, edit, create, refresh, and copy parameter values directly from your terminal.

## Features

- Browse Parameter Store names in a left panel
- Lazy-load values in the right panel (background worker pool)
- Refresh all parameters and values with threaded fetching
- Search/filter parameters with `/`
- Edit selected value in external editor (`$EDITOR` -> `vim` -> `vi`)
- Create new parameter entries from popup (`a`)
- Copy/yank selected value to local clipboard (`y`)
- Shows selected parameter metadata in header

## Requirements

- Rust (stable)
- AWS credentials configured via standard AWS chain
- Network access to AWS SSM APIs
- Optional for clipboard:
  - macOS: `pbcopy`
  - Linux: `wl-copy` or `xclip` or `xsel`

## AWS Configuration

This app uses the standard AWS SDK config chain.

Useful env vars:

- `AWS_PROFILE`
- `AWS_REGION` or `AWS_DEFAULT_REGION`

If region is not set, SDK default chain behavior is used.

## Run

```bash
cargo run
```

## Keybindings

### Main screen

- `/` search/filter mode
- `j` / `Down` move selection down
- `k` / `Up` move selection up
- `R` refresh all parameters and values
- `y` yank/copy selected value to clipboard
- `e` edit selected value in external editor and update SSM on change
- `a` open create-parameter popup
- `q` or `Esc` quit

### Create popup

- `Tab` switch field (name/value)
- `Ctrl+S` save/create
- `Esc` cancel

#### Value field (vim-like)

Insert mode:
- type to insert
- `Enter` newline
- `Esc` to Normal mode

Normal mode:
- `h/j/k/l` move cursor
- `0` / `$` line start/end
- `i` insert before cursor
- `a` insert after cursor
- `x` delete char
- `Enter` submit create
- `Esc` cancel popup

## Project Structure

- `src/main.rs` runtime loop + key handling
- `src/app.rs` app state and core behaviors
- `src/aws_ssm.rs` AWS SSM integration
- `src/ui.rs` all ratatui rendering
- `src/editor_tools.rs` external editor + clipboard helpers
- `src/text_edit.rs` cursor/text editing helpers
- `src/models.rs` shared data types

## Notes

- On startup failure to load from SSM, the app now starts with an empty list (no demo defaults).
- SecureString values are fetched with decryption first, with non-decryption fallback.
