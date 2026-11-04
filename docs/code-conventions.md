# Code conventions

Hya's workspace lints (`Cargo.toml`, `[workspace.lints]`) enforce most of this already; CI fails if they are not followed. This doc says what they add up to, and covers the parts no lint can check: Swift, and writing.

## Rust

- **No `unsafe`.** `unsafe_code = "forbid"`, workspace-wide, including `crates/wasm`.
- **No panics in production code.** `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, and indexing (`v[i]`) are all denied. Every fallible step returns a typed `Result`; every lookup that can fail uses `.get()` or a checked constructor.
- **No silent numeric loss.** `as` casts, `cast_possible_truncation`, `cast_sign_loss`, `cast_possible_wrap`, unchecked arithmetic (`arithmetic_side_effects`), and float `==` are all denied. Convert with `TryFrom`, and compare floats with an explicit tolerance.
- **Invalid states are unrepresentable.** A type like [`Latitude`](../crates/engine/src/input.rs) or [`Date`](../crates/engine/src/date.rs) has private fields; the only way to build one is a checked constructor that rejects NaN, infinity, and out-of-range values. If you need a new bounded input, follow one of those as a template rather than validating by hand at every call site.
- **Errors are plain enums, not a crate.** No `thiserror`, no `anyhow`: the workspace stays `no_std` and dependency-light. An error type is `#[non_exhaustive]`, usually `Copy`, implements `Display` by hand with a lowercase, sentence-fragment message (`"the sun does not rise or set on this day"`, not `"Error: ..."`), and implements `core::error::Error` with no fields to format. See [`ComputeError`](../crates/engine/src/compute.rs) as the template.
- **Doc comments are mandatory on public items** (`missing_docs = "deny"`). Write the *why*, not a restatement of the name: compare `Fajr and Isha: twilight length, bounded by the high-latitude rule` to `the Fajr and Isha times`. A doc comment that only repeats the signature should not exist.
- **A lint gives way only with a reason.** `#[allow(...)]` is denied outright (`allow_attributes = "deny"`); use `#[expect(lint::name, reason = "...")]` instead, so the suppression itself fails to compile the day it stops being necessary.
- `std_instead_of_core`, `std_instead_of_alloc`, and `alloc_instead_of_core` keep `crates/engine` and `crates/countries` buildable for `wasm32-unknown-unknown` with no allocator. `crates/wasm` and `apps/cli` are the only crates allowed to assume a full standard library.

## Swift (`apps/menubar`)

There is no linter for this yet; match the existing files.

- A type's state lives in one `ObservableObject` (`AppState`, `ScheduleController`), published with `@Published`, not scattered across views.
- A function that cannot fail for a structural reason gets a comment saying why, right above the force-unwrap or assumption, not a silent `!` (see `nextLocalMidnight` in `ScheduleController.swift`). A function that can fail returns a typed `Result`, as `HyaProcess.runHya` does with `HyaProcessError`, not a thrown error swallowed somewhere above it.
- Errors are `enum ... Error: Error, Equatable`, with one case per distinct failure, the same shape as the Rust side.

## Comments

Code should explain itself through names and small functions. Write a comment only for a non-obvious *why*: a platform quirk (the `hya-cli` rename, `AppState.swift`), a constraint the type system cannot express (`nextLocalMidnight`'s force-unwrap), or a reference to the evidence behind a constant (the falak-convention comment in `crates/engine/src/compute.rs`). Never write a comment that only restates the line below it. An unusual decision that needs more than a line goes in [`docs/exceptions.md`](exceptions.md) instead, with a short pointer comment at the code.

## Writing

Applies to doc comments, `AGENTS.md`, `docs/`, commit messages, and pull request descriptions:

- Short sentences. One idea each.
- Active voice, present tense: "the engine computes", not "times are computed by the engine" or "will compute".
- One word, one meaning: do not alternate between two words for the same thing in one document (the README says "method", never "profile" or "preset" for the same concept, except where it defines "a method is only a preset" on purpose).
- A number is evidence, not decoration: "528 golden cases", not "many cases". If you cannot cite the number, cite the file that has it.
- No em-dashes. Use a period, a comma, or parentheses.
