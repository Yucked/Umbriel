# Working on Umbriel

Project rules for this repository. They add to the global agent instructions and `rules/rust.md`, and win where they conflict.

Umbriel is a pure Rust ORM for the latest SurrealDB, rebuilt from `surreal_orm`.

## Repository

- `crates/orm/`: the public `umbriel` crate (models, queries, schema, execution). Migrations and their CLI sit behind the `migrations` feature.
- `crates/macros/`: `umbriel-macros`, the derives and query macros.
- `crates/tests/`: `umbriel-tests`. **All tests live here** (see Tests).
  - No `#[cfg(test)]`, `#[test]`, `#[tokio::test]` or test helpers in `src/`. Inherited inline tests move during the restructure; don't add new ones.
  - Private items that tests need are exposed through the `test-support` feature, never made `pub`.

## Toolchain

- `cargo`, `rustfmt` and `gh` run in WSL (Ubuntu), not on the Windows PATH: `wsl.exe -e bash -lc "cd <path> && <command>"`.
- **Git runs natively on Windows, never in WSL.** Commit identity exists only there, and WSL Git reports false CRLF/LF modifications.
- Use only the configured toolchain. No Python, jq, ad-hoc scripts or other tools without approval. Edit files with editor tools, not scripts.

## Permission before acting

- **Ask immediately before the operation.** A plan that mentions a dependency or upgrade isn't approval. This includes `cargo search`.
- **No new persistent databases or instances** without approval; say where they'll be created. Tests never start a SurrealDB of their own (see Tests).
- **Never start, restart or kill long-running processes** such as a SurrealDB server. Ask how to run them.
- **No unrequested automation,** polling, wakeups or background processes.
- **Work only from an issue number the user gives you.** Don't pick the next issue.

## Issue workflow

Issues live in `Yucked/umbriel`. Every change-log entry carries the issue number.

1. Fetch the issue with `gh issue view <N> --comments` and read the relevant code.
2. Draft a plan: design decisions, files affected, tests, verification. Only the tests in the approved plan get written.
3. Post it with `gh issue comment <N> --body-file <file>`, never with the body inline. Post research as a separate comment.
4. **Stop for review.** Re-fetch the comments; the user's edited plan is the specification.
5. Implement, logging each change.
6. Run Verification.
7. **Stop and report.** Don't commit until told to. Each further round needs its own go-ahead.
8. When approved, commit:
   - Don't split hunks with scripts or interactive staging. Commit a mixed file on its own and list each of its changes in the body.
   - A fix to uncommitted work goes into that work's commit.
   - Imperative subject, one body bullet per log entry, `Refs #<N>` trailer. No `Closes`/`Fixes`, no co-author trailer.
9. Post the commit hashes to the issue and stop. Don't close the issue.

## Project rules

- **Scope is a pure ORM:** models, queries, schema, graph, transactions, streaming, migrations. Server-side and operational features are out. Raw SurrealQL doesn't count as support; supported means a typed, documented, tested API.
- **`surreal_orm` isn't the spec.** Keep what works, rewrite what doesn't, and don't carry SurrealDB 1.x semantics forward.
- **Verify SurrealDB behaviour** against the current SurrealDB docs and release notes, and the Rust SDK on docs.rs.
- **Optional engines, transports and migrations stay behind features.**
- **Synthetic data only** in tests, fixtures, examples and docs: Alice/Bob, Acme Corp, `*@example.com`.

## Tests

- **Two test targets only.** `crates/tests/tests/integration/main.rs` declares every integration test module, and `crates/tests/tests/compile.rs` runs trybuild and the fixture crates. trybuild holds compile-fail cases only, so it checks them in one batch; code that must compile is a `compiles_*` module in the integration binary. Never add another top-level `tests/*.rs`: each one is a separate binary that links SurrealDB.
- **The tree mirrors the sources.** The tests of `crates/<crate>/src/<path>.rs` are in `crates/tests/tests/integration/<crate>/<path>.rs`, with `orm` for `umbriel` and `macros` for `umbriel-macros`: e.g. `orm/statements/select.rs`. A module's own tests are in the file named after it (`orm/statements.rs` for `statements/mod.rs`, `orm.rs` for `lib.rs`), which declares its children. A file tests exactly one source module. `server.rs` is the one exception and covers the remote transports.
- **No ignored doctests.** Examples compile and run; use `no_run` where they need a server. `umbriel-macros` can't use `umbriel` in its doctests, so the examples of the macros are on `umbriel`'s re-exports. Illustrations of generated code are `text` blocks.
- **A parameterised case** is added only when it covers a distinct contract or code path.
- **Snapshots** only for large generated SurrealQL, as a projection, never a `Debug` dump.
- **Helpers** live in `crates/tests/src/`, and only once a second file needs them: the harness in `lib.rs`, the migration helpers in `migrations/helpers.rs`, and the names and fixtures one area's files share in `support/<area>.rs`, imported with a glob.
- **Fixtures** live in `crates/tests/tests/integration/fixtures/`, named in `snake_case`.
- **No embedded or private SurrealDB.** Database tests run against developer-started servers (`crates/tests/README.md`), each in a database of its own that the harness removes. They are never `#[ignore]`d: a missing server fails them. This overrides the `#[ignore = "needs X"]` rule in `rules/rust.md`.

## Verification

In order, through WSL (DO NOT RUN THESE YOURSELF):
1. `cargo build --workspace --all-targets`
2. `cargo clippy --workspace --all-targets`: no new warnings. Baseline: none recorded yet (set after milestone 2).
3. `rustfmt <changed files>`
4. `cargo test --workspace`: check failures and the ignored count.
5. Native Windows `git status --porcelain=v1`: revert anything unrelated.
6. No `cfg(test)`, `#[test]` or `#[tokio::test]` in changed `src/` files.

During milestone 1 the workspace isn't expected to build: report the remaining errors instead of steps 1–4, and confirm they come from the planned migration.
