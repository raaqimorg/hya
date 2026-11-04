# Exceptions

Hya is meant to look boring: one pure function, three ordinary ways to call it. Anything that does not, needs a reason written down here, so the next person (human or agent) does not try to "fix" it. See `AGENTS.md`, "Keep it boring".

This list is short on purpose. If you find code that looks unusual and is not here, say so; either it should be, or the code should change.

## The menu bar app shells out to the CLI instead of linking the engine

`apps/menubar` is Swift, and `crates/engine` is Rust. The ordinary way to bridge them is a C ABI: a `cdylib`, a generated header, and a bridging layer (by hand, or through `cbindgen` plus something like `swift-bridge` or UniFFI).

Hya does it the other way: `AppState.bundledExecutableURL` finds a copy of the `hya` CLI binary bundled next to the app (renamed `hya-cli`; see the next entry), and `HyaProcess.runHya` spawns it with `Process`, passing `--json --lat --lng`, and decodes the one line of JSON it prints (`apps/menubar/Sources/Hya/HyaProcess.swift`).

- The app calls `compute` at most once a minute (`ScheduleController`). A process spawn is not a hot path here.
- It needed no FFI toolchain, no generated bridging header, and no change to `crates/engine`'s `no_std` build to add one more consumer.
- The CLI's `--json` output is already a tested, stable contract (`apps/cli/src/json.rs`, `crates/engine/tests/`), so the menu bar app gets the same guarantees as every other door in [`docs/topology.md`](topology.md) for free, instead of a second, Swift-side copy of them.
- The cost: a `HyaProcessError` the CLI can produce (`notFound`, `nonZeroExit`, `invalidOutput`) that FFI would not have, and one process launch per refresh.

If the menu bar app ever needs the engine on a much hotter path, or on a platform where bundling a second binary is not possible, FFI is the standard answer and this entry should be replaced, not extended.

## The bundled CLI binary is named `hya-cli`, not `hya`

`apps/menubar/Scripts/package-dmg.sh` copies the release `hya` binary into `Hya.app/Contents/MacOS/` under the name `hya-cli`. macOS's default APFS format is case-insensitive but case-preserving, so a file named `hya` there would collide with the app's own executable, `Hya` (`AppState.swift`, the comment on `bundledExecutableURL`). `HyaProcess` and `AppState` hard-code the renamed path.
