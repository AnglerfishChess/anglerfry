# Anglerfry

A small, readable UCI chess engine in Rust, to build your own on top of.

*Anglerfry is what Anglerfish was before it learned to hunt.*

It speaks the protocol properly and plays badly on purpose: the interesting part is the shape — a
stdin loop that keeps answering while a search thread thinks — not the moves it picks.

## Build and run

```sh
cargo build --release
./target/release/anglerfry
```

It then reads UCI commands on stdin; type `uci` to start. Set `RUST_LOG=debug` for a trace on
stderr.

## Use from a GUI

Add `target/release/anglerfry` as a UCI engine in any chess GUI (Arena, Cute Chess, BanksiaGUI,
En Croissant). It needs no arguments and no working directory.

The `Strategy` option chooses how moves are picked:

- `random` — a uniformly random legal move (the default).
- `two-ply` — a shallow negamax over material.

## Add a strategy

1. Write `src/strategy/yours.rs` with
   `pub fn pick(board: &Board, limits: &Limits) -> Option<Move>`, returning once
   `limits.spent(nodes)` says so and calling `report` at least once before it does.
2. In `src/strategy/mod.rs`: declare the module, add a variant to `Strategy`, and extend `ALL`,
   `name` and `pick`.

The option line and the GUI's combo box follow from `ALL`.

## Licence

MIT, see [LICENSE](LICENSE).
