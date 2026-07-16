# pkg/boxlayout

Recursive box-model layout algorithm used to arrange terminal UI windows into a grid of
non-overlapping regions. Source: `pkg/boxlayout/boxlayout.go`.

## Core idea

Windows are laid out by arranging a tree of `Box` values in the available space. Each `Box` with
children specifies how those children are stacked: `ROW` (children stacked vertically) or `COLUMN`
(children stacked horizontally). A leaf `Box` maps to a named window via its `Window` field.

Sizing a box's children happens in two passes:
1. Boxes with a static `Size` get exactly the space they ask for (cropped if not enough is available).
2. Remaining space is divided among boxes with a `Weight`, proportionally. E.g. weights 1 and 2 split
   the leftover space 33% / 66%.

A box can't define both `Size` and `Weight` — pick one.

## Types

```go
type Dimensions struct {
    X0, X1, Y0, Y1 int
}

type Direction int
const (
    ROW Direction = iota
    COLUMN
)

type Box struct {
    Direction            Direction
    ConditionalDirection  func(width, height int) Direction
    Children              []*Box
    ConditionalChildren    func(width, height int) []*Box
    Window                string // name of the window this leaf box represents
    Size                  int    // static size (height if parent is ROW, width if COLUMN)
    Weight                int    // dynamic share of remaining space
}
```

`ConditionalDirection` / `ConditionalChildren` let a box's layout depend on the width/height it's been
assigned (e.g. switch from side-by-side to stacked panels below a terminal width threshold).

## Entry point

```go
func ArrangeWindows(root *Box, x0, y0, width, height int) map[string]Dimensions
```

Walks the tree recursively, returning a flat map of window name → `Dimensions` (absolute
top-left/bottom-right coordinates).

## Sizing algorithm (`calcSizes`)

1. Normalize weights to their smallest integer ratio (`normalizeWeights`/`calcFactors`) — e.g. `2,4,4`
   becomes `1,2,2` — to avoid rounding error building up.
2. Sum the static sizes (`reservedSpace`) and the total normalized weight.
3. `dynamicSpace = max(0, availableSpace - reservedSpace)`.
4. `unitSize = dynamicSpace / totalWeight`, with `extraSpace = dynamicSpace % totalWeight` distributed
   one unit at a time across boxes with nonzero weight (round-robin) so the remainder isn't lost to
   integer division.

## Porting notes for ssmtui (Rust)

- `Box` maps naturally to an enum/struct with `Option<Vec<Box>>` children and either a fixed size or a
  weight (an enum `Size::Static(u16) | Size::Weight(u16)` would enforce the "pick one" constraint that
  Go can't).
- The conditional-direction/children callbacks are just `Fn(u16, u16) -> ...` closures — straightforward
  to translate.
- The remainder-distribution loop in `calcSizes` is the trickiest bit to get right; keep the round-robin
  distribution so weighted panes come out within 1 cell of ideal size rather than the last one absorbing
  all rounding error.
