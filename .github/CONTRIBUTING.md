# Contributing

## Start with an issue

Every pull request links to an [issue](https://github.com/raaqimorg/hya/issues). To report a bug or an idea that you do not plan to work on, the form asks only for what happened and its impact. For a security vulnerability, see [SECURITY.md](SECURITY.md) instead.

If a time is wrong for your city, use the **Timetable mismatch** form. It asks for the place, the date, the method, what Hya printed, and what the authority publishes, with a link to the authority's own timetable. A mosque's printed calendar or another app is useful context, but only the authority's own publication can change a preset (README, "Methods left out, and why").

If you plan to send the fix, open the issue first, or pick an existing one. Then fill in the rest of the form:

- **Impact:** who gets a wrong time, and by how much.
- **Root cause:** the code or the parameter that causes it.
- **Proposed fix:** a snippet or a clear description.
- **Reasoning:** why the fix solves the underlying problem, not one city or one day.
- **Trade-offs:** whether the fix can move times elsewhere. Say which methods, latitudes, and seasons the changed code touches.

For a large or significant change, such as a new method, a new high-latitude rule, or a change to the public API, wait until the maintainers agree on the approach in the issue before you write code. Keep each pull request small, with one concern.

## Adding or changing a method

A method is a preset of parameters, so most of the work is evidence, not code.

1. Find the authority's own published times: a timetable, a calculator, or an open dataset. Third-party sites that quote angles are not enough.
2. Save them as a fixture with `bun run fixtures:official`, or add a fetcher to [`scripts/fetch-official.ts`](../scripts/fetch-official.ts). Record the source URL and the date of the download in the fixture.
3. Fit the parameters, then add a test to [`crates/engine/tests/official.rs`](../crates/engine/tests/official.rs). The test states the tolerance, and it shows that a degree either way fails, so it can tell the right angle from a near one.
4. Add the method to `Method::ALL`, to `METHODS` in the JS package, and to the methods table in `README.md`. If the method is a country's own authority, add the country to [`crates/countries`](../crates/countries/src/lib.rs) with the authority's name.

## Making changes

1. Fork the repo, and create a branch off `main`.
2. Follow the conventions that the lints enforce (`Cargo.toml`, `[workspace.lints]`). Production code has no `as`, no unchecked arithmetic, no indexing, no `unwrap`, and no `#[allow]`. Use `#[expect(..., reason = "...")]` where a lint must give way, and say why.
3. Add a test for every change in behavior. A new invariant goes in [`tests/properties.rs`](../crates/engine/tests/properties.rs). A fixed bug gets a test that fails without the fix.
4. Run `bun run ci` before you open a pull request. It runs format, clippy, the Rust tests, the `no_std` build, the Ramadan table check, and the JS build, types, and tests. GitHub Actions runs the same steps on every push and pull request.
5. If you change the calculation, also run the deep checks that cover it (see below). Say in the pull request which ones you ran.
6. Update `README.md` if you changed what it describes.
7. Open the pull request into `main`. Put `Closes #<issue>` in its description, and fill in the template.

## Fast checks and deep checks

The rule: **CI stays fast.** Every check that runs on every push and pull request finishes in a few minutes. A slow check is never added to CI. It becomes a `deep:` script, which runs on demand.

| Script                    | What it does                                                             | When to run it                                                      |
| ------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------- |
| `bun run ci`              | The fast gate: everything that CI runs                                   | Before every pull request                                           |
| `bun run deep:properties` | Every property test with 1,000,000 cases, not 2,000                      | After a change to the calculation                                   |
| `bun run deep:mutants`    | Mutation testing of `hya-engine` and `hya-countries`. None may be missed | After a change to the calculation. Add `-f <file>` to test one file |
| `bun run deep:kani`       | Kani proofs for both libraries                                           | After a change to integer, date, or parsing code                    |
| `bun run deep:fuzz`       | libFuzzer for 10 minutes (`FUZZ_SECONDS` changes it)                     | After a change to validation or the calculation                     |
| `bun run deep:coverage`   | An HTML coverage report                                                  | When you look for untested code                                     |
| `bun run deep`            | All of the checks above except coverage                                  | Before every release                                                |

Each one needs its tool: `cargo install cargo-mutants`, `cargo install kani-verifier && cargo kani setup`, `cargo install cargo-fuzz` with a nightly toolchain, and `cargo install cargo-llvm-cov`. To run them on GitHub instead, open Actions, select **deep**, and select **Run workflow**. That workflow also runs the tests on macOS and Windows.

The git hooks are fast too. `bun install` turns them on (`core.hooksPath = .githooks`). The pre-commit hook checks formatting, and the pre-push hook runs clippy.

## Releasing

Maintainers release from `main`. Versions follow [Semantic Versioning](https://semver.org), and one version number covers the crates and the npm package.

1. Run `bun run deep`. Every check must pass.
2. Set the new version in `Cargo.toml` (`[workspace.package]` and the `hya-engine` and `hya-countries` dependencies) and in `packages/js/package.json`. Then run `cargo check` to update `Cargo.lock`.
3. In `CHANGELOG.md`, move the entries under `[Unreleased]` into a new `## [x.y.z] - YYYY-MM-DD` section, and add its link at the bottom.
4. Run `bun run release:check x.y.z`. It fails if any version disagrees, and otherwise prints the release notes.
5. Commit with the message `Release x.y.z`, and push it to `main`.
6. Tag the commit and push the tag: `git tag -a vx.y.z -m "Hya x.y.z"`, then `git push origin vx.y.z`.

The tag starts [`release.yml`](workflows/release.yml). It checks the versions again, runs the fast gate, and builds the CLI for Linux, macOS, and Windows. Then it publishes `hya-engine`, `hya-countries`, and `hya-cli` to crates.io and `@raaqimorg/hya` to npm, and creates the GitHub Release with the binaries, their SHA-256 checksums, and the changelog section. It publishes with trusted publishing, so no registry token is stored anywhere. A version that is already published is skipped, so a failed release can be re-run from the Actions tab.

The first release is different: crates.io and npm accept trusted publishing only for a package that already exists. So a maintainer publishes 0.1.0 by hand with a token, in the order `hya-engine`, `hya-countries`, `hya-cli`, then `npm publish --access public` in `packages/js`. Then, on each package's settings page, they add a trusted publisher: the repository `raaqimorg/hya`, the workflow `release.yml`, and the environment `release`. Pushing the `v0.1.0` tag then creates the GitHub Release, and skips the publishing.

## Working with an AI agent

Install the [GitHub CLI](https://cli.github.com) (`gh`), and run `gh auth login`. With it, an agent can do these tasks without copying text between the terminal and the browser:

- Read an issue and its discussion: `gh issue view <number> --comments`.
- Open an issue: `gh issue create`.
- Open a pull request that links to the issue: `gh pr create`.

An agent must not change a fixture by hand to make a test pass. Fixtures come from the authority or from adhan-js, through the scripts in `scripts/`.

## Keeping your email private

Every commit carries an author email, and a push publishes it. Use these steps to make this clone commit only with the address that you choose. For example, use a GitHub no-reply address, not a personal or work address.

1. Set that address for this clone only: `git config --local user.email <address>`.
2. Turn on the guard: `git config --local hya.allowedEmail "$(git config --local user.email)"`.
   - The git hooks then refuse every commit and every push that uses a different address as author, committer, or `Co-authored-by:` line. This includes an address that an AI agent or a script sets.
   - The address stays in `.git/config`. It is never committed.
3. On GitHub, go to Settings, then Emails. Turn on "Keep my email addresses private" and "Block command line pushes that expose my email".

## License

By contributing, you agree that your code and documentation contributions are licensed under the project's [MIT license](../LICENSE).
