# pkg/utils

Small shared helpers. Source: `pkg/utils/utils.go`, `pkg/utils/once_writer.go`.

## utils.go

```go
func Min(x, y int) int
func Max(x, y int) int
func Clamp(x int, min int, max int) int
```

Plain integer min/max/clamp — pre-Go-1.21 helpers (stdlib `min`/`max` now cover this in Rust terms
`i32::min`/`.clamp()` already do the job).

```go
func GetLazyRootDirectory() string
```

Walks up from the current working directory looking for a `.git` folder, panicking (`log.Fatal`) if it
hits `/` without finding one. Used by lazygit/lazydocker's cheatsheet scripts and integration tests to
locate the repo root — not a generic "find any repo root" utility.

## once_writer.go

```go
type OnceWriter struct { ... }

func NewOnceWriter(writer io.Writer, f func()) *OnceWriter
func (self *OnceWriter) Write(p []byte) (n int, err error)
```

Wraps an `io.Writer` so that a given callback `f` runs exactly once, right before the first `Write`
call goes through (via `sync.Once`). Useful for lazy setup that should only happen if something is
actually about to be written (e.g. opening a log file only once real output occurs).

## Porting notes for ssmtui (Rust)

- `Min`/`Max`/`Clamp` aren't needed — use `std::cmp::min/max` or `i32::clamp`.
- `GetLazyRootDirectory` isn't lazygit/lazydocker-specific logic worth porting as-is; if ssmtui needs a
  "find repo root" helper, walk up checking for `.git` the same way, but name/scope it to ssmtui.
- `OnceWriter` maps to a struct wrapping any `Write` implementor plus a `std::sync::Once`, calling the
  callback inside `Once::call_once` before delegating to the inner writer's `write`.
