---
name: roadmap-row
description: Procedure for implementing exactly one row of a roadmap table in docs/Roadmaps. Use whenever asked to do, continue or finish a roadmap row such as "0001.7f" or "the next row".
---

# Work one roadmap row

Do these steps in order. Do not skip one, and do not work on two rows at once.

## 1. Find the row

1. Open the roadmap file in `docs/Roadmaps/`.
2. The row to work on is the first row without `✅`, unless the user named one.
3. Check the "Depends on" cell. If that row has no `✅`, stop and tell the user.

## 2. Read before writing

1. Read the row's Task and Done-when cells completely.
2. Read the roadmap section "The model and decisions". The exact type and function names are there.
3. Read the code the row touches. Read whole files, they are small.
4. Load the skill the row names after "Skill:".

## 3. Test first

1. Write the tests the row names. Give each test the name stated in the row.
2. Run `cargo test -p <crate> <test name>` and see it fail for the right reason (missing function or
   wrong value, not a typo).

## 4. Implement

1. Write only what the row asks for. No extra options, traits, modules or refactors.
2. Follow the rules in `CLAUDE.md`.
3. If a crate function named in the row does not exist, look it up in the docs of the installed
   version. Do not guess a name and do not downgrade the crate.

## 5. Gates

Run all three. All must be clean.

```bash
cargo fmt --all
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo test --workspace
```

## 6. Check "Done when" by hand

1. Do exactly what the Done-when cell describes: run the command, or start the app and click through
   the scenario.
2. For screen rows, start the app with the sample book:

```bash
cargo run -p pagecrop -- samples/book
```

3. If there is no display or the step cannot be performed, say so plainly. Never report a check as done
   that was not performed.

## 7. Close the row

1. Put `✅` in front of the row id in the roadmap table.
2. Commit everything with the message `<row id>: <short summary>`.
3. Report in three lines: what was built, what was checked and how, anything that differs from the row.

## When to stop and ask

- The row contradicts the code or another row.
- A gate fails and the fix would change a function an earlier row defined.
- The row needs more than about 300 new lines. It is probably two rows.
- "Done when" cannot be made true.

Stopping with a clear question is correct. Inventing a different design is not.
