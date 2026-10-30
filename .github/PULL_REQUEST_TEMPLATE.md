Closes #

<!-- Give the issue that this PR resolves. If there is no issue, open one first (see CONTRIBUTING.md). -->

## What changed

<!-- Write one or two sentences: what this PR does, and why. -->

## Evidence

<!-- For a change to any time: which official timetable or fixture shows it is right, and how far the times moved (for example, "Makkah 2026: exact 87.6% → 89.0%, max error 2 → 1 min"). If no time moves, delete this section. -->

## Checks

- [ ] The linked issue covers the impact, root cause, proposed fix, reasoning, and trade-offs. For a larger change, the maintainers agreed on the approach there.
- [ ] This PR does one thing.
- [ ] `bun run ci` passes locally (CI runs the same steps).
- [ ] For a change to the calculation: the deep checks that cover it pass (`deep:properties`, `deep:mutants`, and `deep:kani` for integer or date code). Name them in the notes.
- [ ] No fixture was edited by hand. Fixtures come from the scripts in `scripts/`.
- [ ] `README.md` is updated, if it describes what changed.

## Notes for the reviewer

<!-- Write anything that is not obvious: a trade-off that you made, something that you left out on purpose, or a follow-up that you plan. If there is nothing, delete this section. -->
