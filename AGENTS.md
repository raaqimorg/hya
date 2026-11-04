# Hya Agent Guidelines

This repo holds Hya: Islamic prayer times from coordinates alone, with no network, as a Rust library, a JavaScript package, a `hya` command, and a SwiftUI menu bar app. Turborepo and Bun build the Rust-and-TypeScript workspace; the menu bar app is a separate Swift package.

## Layout

- `crates/engine` (`hya-engine`): the calculation. Pure, `no_std`, no I/O, no clock, no allocation. Every other piece exists to call this one function, `compute`.
- `crates/countries` (`hya-countries`): country code to method, where that country's own authority has one.
- `crates/wasm` (`hya-wasm`): WebAssembly bindings over `crates/engine`. Numbers in, numbers out; no logic of its own.
- `packages/js` (`@raaqimorg/hya`): the TypeScript API, with the compiled WebAssembly inlined and the inputs branded the same way the Rust types are.
- `apps/cli` (`hya`): the command. Prints today's times; asks for a place once and saves it; `--json` is the stable, tested contract that `apps/menubar` also relies on (`apps/cli/src/json.rs`).
- `apps/menubar`: `Hya.app`, the macOS menu bar app. SwiftUI and AppKit, its own Swift package (`Package.swift`), outside the Bun workspace. It does not link `crates/engine`; it spawns the bundled `hya` CLI (renamed `hya-cli`) as a subprocess and reads its `--json` output. See [`docs/exceptions.md`](docs/exceptions.md) for why.
- `fuzz/`: a `cargo-fuzz` target over `crates/engine`, in its own Cargo workspace, nightly only.
- `fixtures/`: golden data from adhan-js and from official timetables, checked by `crates/engine/tests/` and `crates/countries/tests/`. Never hand-edit a fixture; regenerate it with the `scripts/` that produced it.
- `scripts/`: the fixture and Ramadan-table generators, the official-timetable fetcher, and the release-version check, each with one `bun run` name.
- `docs/`: everything that is not a doc comment or the README:
  - [`topology.md`](docs/topology.md): a diagram-first map, with the diagrams in `docs/architecture/*.svg`.
  - [`code-conventions.md`](docs/code-conventions.md): what the lints enforce, Swift conventions, comments, and writing.
  - [`exceptions.md`](docs/exceptions.md): approved unusual code, and why.

## Commands

```bash
bun run ci              # the fast gate: fmt, clippy, Rust tests, no_std build, Ramadan check, JS build/types/test
cargo test --workspace --locked   # just the Rust tests
bun run deep             # everything slow, before a release (see CONTRIBUTING.md, "Fast checks and deep checks")
```

`bun run ci` does **not** build or test `apps/menubar`; it is not a Bun workspace. After touching it:

```bash
cd apps/menubar && swift build && swift test
```

Full list of scripts: `package.json`. Full contributor workflow, including adding a method and releasing: [`.github/CONTRIBUTING.md`](.github/CONTRIBUTING.md).

## Keep it boring

- The calculation is the one place correctness is worth real cost: proofs, mutation testing, 1,000,000-case property runs. Everywhere else, write what an experienced Rust (or Swift) developer expects in any similar project.
- Treat anything unusual as a warning sign: a custom numeric type where a checked constructor would do, a new dependency for something `libm`, `jiff`, `clap`, or the standard library already does, indexing instead of a checked lookup, a clever one-liner instead of two clear ones.
- If a request would add unusual code, do not build it yet. Say plainly that it is unusual and why, and show the boring version. Then wait for the user's decision.
- Read [`docs/exceptions.md`](docs/exceptions.md) before you build. Anything it does not list must be standard. When the user approves an unusual approach, add its entry there in the same change.

## Agent workflow

- Hard rule: never run `bun run deep:*`, `cargo kani`, or `cargo fuzz` unless the user asks for that specific check. They are slow and some need extra tools; `bun run ci` is the default.
- Hard rule: never tag, publish, or run anything in `release.yml` or `apps/menubar/Scripts/package-dmg.sh` unless the user asks for that release. Approval for one release does not carry over to the next.
- An agent must not edit a fixture under `fixtures/` by hand to make a test pass. A fixture comes from `scripts/fetch-official.ts`, `scripts/generate-adhan-js.ts`, or `scripts/generate-asr-reference.ts`; change the generator, or the method's parameters, not the data.
- Changing the calculation (`crates/engine/src/`) needs a new test, almost always in `crates/engine/tests/properties.rs` for an invariant or `tests/official.rs`/`tests/golden.rs` for a case. Say in the pull request which `deep:*` checks you ran, if any.
- Install the [GitHub CLI](https://cli.github.com) (`gh`) for issues and pull requests; see [`.github/CONTRIBUTING.md`](.github/CONTRIBUTING.md).
- Commits: see [`.github/CONTRIBUTING.md`](.github/CONTRIBUTING.md), "Keeping your email private". The `commit-msg` and `pre-push` hooks reject any author, committer, or `Co-authored-by:` email but the one `hya.allowedEmail` names.

## Style

- Read [`docs/code-conventions.md`](docs/code-conventions.md) before you write or review Rust or Swift, and before you write docs, a commit message, or a pull request description.
- Read [`docs/exceptions.md`](docs/exceptions.md) before you build something that does not look like the rest of the codebase.
- Update `README.md` if you change what it describes (it is the one place usage, the calculation, edge cases, methods, and verification are documented for humans; `docs/` is architecture and process, not user-facing usage).
