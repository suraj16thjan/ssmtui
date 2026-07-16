# lazycore

Reference notes on [jesseduffield/lazycore](https://github.com/jesseduffield/lazycore) — shared Go
functionality used by lazygit and lazydocker. Kept here as documentation for porting ideas into ssmtui
(Rust), not as a build dependency.

Source repo layout:
- `pkg/boxlayout` — recursive box-model layout algorithm for arranging terminal windows. See [boxlayout.md](boxlayout.md).
- `pkg/utils` — small shared helpers (`Min`/`Max`/`Clamp`, `GetLazyRootDirectory`, `OnceWriter`). See [utils.md](utils.md).

Real-world consumer: [jesseduffield/lazygit](https://github.com/jesseduffield/lazygit) uses both
packages — see [howtouselazycore.md](howtouselazycore.md) for how.

License: MIT (see upstream repo).
