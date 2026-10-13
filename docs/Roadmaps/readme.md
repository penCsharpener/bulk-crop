# Roadmaps

One file per feature or fix, named `NNNN Title.md`. Each holds a task table that is worked top to
bottom.

## Reading a task table

| Column | Meaning |
|---|---|
| `#` | Row id, `NNNN.n`. A trailing `f` marks the screen row for the core row of the same number: `0001.6f` is the UI for `0001.6`. `f1`, `f2` are two screen rows for one core row. A trailing letter such as `0001.7b` is a row inserted between two others. |
| Task | What to build: files, function names, rules. |
| Depends on | The row that must be finished first. `—` means none. |
| Done when | What a person can observe when the row is finished. |

A core row is not finished in spirit until its `f` row is finished, because only then can the change be
seen and clicked.

Rows without an `f` partner say in the task cell whether they are core plus command line, or screen
only.

## Working a row

Load the `roadmap-row` skill and follow it. In short: read the row and the rows it depends on, write the
named test, write the code, run the three gates from `CLAUDE.md`, check "Done when" by hand, commit,
then mark the row by putting `✅` in front of its id.

## Rules for the table

- The app runs after every row. If a row cannot keep that promise, the roadmap's Risks section says so.
- Do not start a row whose dependency is not marked `✅`.
- Do not do work that belongs to a later row, even when it looks convenient.
- When a row turns out to be wrong or too big, stop and say so. Do not improvise a different design.
