# How lazygit uses lazycore

Notes from grepping [jesseduffield/lazygit](https://github.com/jesseduffield/lazygit) for
`github.com/jesseduffield/lazycore`, to see how the library described in [README.md](README.md),
[boxlayout.md](boxlayout.md) and [utils.md](utils.md) is actually consumed in a real app.

## Dependency

`go.mod` pins a plain module version (not a `replace` directive — lazycore is a normal published
dependency, also vendored under `vendor/github.com/jesseduffield/lazycore/`):

```
github.com/jesseduffield/lazycore v0.0.0-20221012050358-03d2e40243c5
```

## `pkg/boxlayout` — the entire window layout engine

This is the main real-world usage. Lazygit's whole panel layout (file list, branches, commits, main
view, status line, etc.) is one big `boxlayout.Box` tree, rebuilt on every resize/redraw.

**`pkg/gui/controllers/helpers/window_arrangement_helper.go`** owns this. Key points:

- `WindowArrangementArgs` is a plain struct capturing everything the layout depends on — screen width/
  height, user config, which window/side is focused, screen mode (normal/half/full), whether a mode
  (rebase, cherry-pick) is active, search state, etc. It's assembled once per layout pass from live app
  state (`WindowArrangementHelper.GetWindowDimensions`), then passed into pure functions.
- `GetWindowDimensions(args) map[string]boxlayout.Dimensions` builds the `root *boxlayout.Box` tree and
  calls `boxlayout.ArrangeWindows(root, 0, 0, args.Width, args.Height)` — this is the single call site
  that invokes the lazycore algorithm. It's called a second time with a trivial one-box tree just to get
  a "limit" window's dimensions, and the two resulting maps are merged with `MergeMaps`.
- The tree mixes static (`Size`) and dynamic (`Weight`) boxes, matching the size-then-weight algorithm
  in `boxlayout.md`:
  - Root: `ROW` with two children — the main body (`Weight: 1`) and an info/status line
    (`Size: infoSectionSize`, either 0 or 1 depending on `showInfoSection`).
  - The body splits into a side-panel column (`Weight: sideSectionWeight`) and a main-panel column
    (`Weight: mainSectionWeight`), with `sidePanelsDirection` swapped between `COLUMN` and `ROW` for
    portrait mode (`shouldUsePortraitMode`).
  - Leaf boxes correspond 1:1 with named UI windows (`Window: "main"`, `"extras"`, `"appStatus"`,
    `"options"`, etc.), e.g. `&boxlayout.Box{Window: "extras", Size: getExtrasWindowSize(args)}`.
- **`ConditionalChildren`/`ConditionalDirection` are used heavily** to make the layout responsive to the
  size it's assigned rather than globally-known state. `sidePanelChildren(args)` returns a
  `func(width, height int) []*boxlayout.Box` closure (an "accordion" mode where the focused side-panel
  window expands and others shrink to a fixed size once space is tight); `mainSectionChildren` similarly
  branches on `SplitMainPanel` and screen mode.
- Small helper boxes are built with closures for DRY-ness, e.g. `spacerBox()` (`Size: 1`) and
  `flexibleSpacerBox()` (`Weight: 1`) for filling gaps in the bottom status line, and
  `accordionBox(defaultBox)` for the expand/collapse-on-focus behavior of side panels.
- `shrinkToContentSidePanelBoxes` is a more advanced case: it sizes panels to their actual content
  height (via `args.ContentHeightForWindow`) when there's enough room, falling back to weighted
  boxes when content doesn't fit — built on top of `boxlayout.Box{Size: ...}` vs `{Weight: ...}` per
  panel, computed dynamically rather than statically declared.

Call chain into the actual UI: `gui.go`'s `getWindowDimensions` (a thin one-line delegator) →
`WindowArrangementHelper.GetWindowDimensions` → package-level `GetWindowDimensions(args)` →
`boxlayout.ArrangeWindows`. The resulting `map[string]boxlayout.Dimensions` is what the gocui-based
renderer uses to actually position each named view on screen.

Tests: `window_arrangement_helper_test.go` builds `WindowArrangementArgs` fixtures, calls
`GetWindowDimensions`, and renders the resulting `map[string]boxlayout.Dimensions` back into ASCII art
(`renderLayout`) to snapshot-test layouts across screen sizes/config combinations — a good pattern to
copy if ssmtui ever wants layout regression tests.

## `pkg/utils` — repo-root discovery only

Every other lazycore usage in lazygit is `utils.GetLazyRootDirectory()` (aliased `lazycoreUtils` in one
file to avoid clashing with lazygit's own `pkg/utils`), used purely to locate the project root so a
tool can find files relative to it regardless of the working directory it was invoked from:

- `pkg/cheatsheet/generate.go` — `utils.GetLazyRootDirectory() + "/docs-master/keybindings"`
- `pkg/jsonschema/generate.go` — `utils.GetLazyRootDirectory() + "/schema-master"`
- `pkg/jsonschema/generate_config_docs.go` — `utils.GetLazyRootDirectory() + "/docs-master/Config.md"`
- `pkg/integration/clients/cli.go`, `go_test.go`, `tui.go` — `tests.GetTests(utils.GetLazyRootDirectory())`
  to discover integration tests relative to the repo root regardless of cwd
- `pkg/integration/components/runner.go` — `lazycoreUtils.GetLazyRootDirectory()` then `os.Chdir` into it
  before building/running the binary under test

`utils.Clamp` shows up once, in the integration-test TUI picker
(`pkg/integration/clients/tui.go:227`), to keep a list-selection index in bounds:
`self.itemIdx = utils.Clamp(self.itemIdx, 0, len(self.filteredTests)-1)`.

`Min`/`Max`/`OnceWriter` aren't used anywhere in lazygit as of this snapshot (`f`) — lazygit vendors
the whole `pkg/utils` package but only exercises `GetLazyRootDirectory` and `Clamp` from it; the rest
presumably serves lazydocker or other consumers.

## Takeaways for ssmtui

- `boxlayout` earns its keep when the layout genuinely depends on runtime state (focus, screen mode,
  content size) — lazygit never hand-computes coordinates, it always re-derives the whole tree from an
  args struct and re-runs the algorithm. If ssmtui's layout is closer to "few fixed panes," a straight
  `ratatui::layout::Layout` (which already does size/weight splits nearest to `Constraint::Length` /
  `Constraint::Ratio`) may already cover this without porting `boxlayout` at all.
- The one part worth stealing regardless of whether the whole engine is ported: driving layout off a
  plain args struct assembled once per frame, and snapshot-testing the resulting dimensions with an
  ASCII renderer (`window_arrangement_helper_test.go`'s `renderLayout`) — cheap regression protection
  for any TUI layout code, Rust or Go.
- `GetLazyRootDirectory` is only useful for lazygit/lazydocker's own build/test tooling (finding the repo
  root for cheatsheet/schema generation and integration tests) — not something ssmtui needs unless it
  grows equivalent codegen or integration-test tooling that must work regardless of invocation cwd.
