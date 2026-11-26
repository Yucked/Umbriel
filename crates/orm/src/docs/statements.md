Typed SurrealQL statements: writes and their return modes, `SELECT`, control flow, and schema
definitions.

Every statement renders to parameterised SurrealQL: values are bound, never written into the
text. A statement runs through [`Returning::execute`](crate::statements::Returning::execute),
which decodes what its return clause returns, or [`Runnable::run`](crate::Runnable::run), which
returns the checked raw results. Both borrow an [`Executor`](crate::Executor). A statement that
can't be built correctly fails before it is sent, with
[`Error::QueryBuilder`](crate::Error::QueryBuilder).

# Writes and return modes

`create`, `insert`, `insert_relation`, `update`, `upsert`, `relate` and `delete` build typed
writes. Their `_only` forms (`create_only`, `update_only`, …) write `ONLY`. A write's return
method sets both its `RETURN` clause and the type `execute` returns:

| Method | `CREATE` | `INSERT` | `UPDATE`/`UPSERT` | `RELATE` | `DELETE` |
| --- | --- | --- | --- | --- | --- |
| default | `Vec<T>` | `Vec<T>` | `Vec<T>` | `Vec<T>` | `()` |
| `return_none()` | `()` | `()` | `()` | `()` | — |
| `return_null()` | `Vec<types::SurrealNull>` | same | same | same | same |
| `return_before()` | — | — | `Vec<T>` | after `or_update()` | `Vec<T>` |
| `return_after()` | `Vec<T>` | `Vec<T>` | `Vec<T>` | `Vec<T>` | — |
| `return_diff()` | `Vec<Vec<DiffOp>>` | same | same | same | same |
| `return_value(field)` | `Vec<V>` | `Vec<V>` | `Vec<V>` | `Vec<V>` | — |
| `return_fields::<P>(…)` | `Vec<P>` | `Vec<P>` | `Vec<P>` | `Vec<P>` | — |

With `ONLY`, the result is `Option<_>` instead of `Vec<_>`. A result that is itself an array,
such as a diff or an array field's `return_value`, decodes whole at any length. A dash means the
mode isn't offered, because SurrealDB documents it as `NONE` there or it has no meaning.

`return_null()` gives one `NULL` per record the statement returns. A `WHERE` that matches
nothing, an `IGNORE`d duplicate, or a record the session may not select returns none.

`explain()` on `UPDATE`, `UPSERT` and `DELETE` returns the plan and writes nothing.
`explain_full()` runs the statement to measure it, so it **performs the write**.

**Ids are checked before a write runs.** `CONTENT` carries the model's record id, so `CREATE`,
`UPDATE` and `UPSERT` fail before they run when that id is outside a table target or differs from
a record target. `RELATE` checks its `CONTENT` id, `in` and `out` against the ids its path was
built from: the edge record, or the step's edge table, and the endpoints. Parameter and subquery
parts are left to the database.

**Several models at once.** A `#[derive(Models)]` enum types `create_any` and `insert_any` over
several models. They offer `NONE`, `NULL`, `AFTER` and `DIFF` returns, because the result has no
single table to decode into.

# Queries

`select(projections).from(targets)` builds a `SelectStatement<O, C>`, where `O` is what each
result decodes into and `C` is whether it is written with `ONLY`. `execute` returns `Vec<O>`, or
`Option<O>` after `from_only`.

| Built with | `O` | What the type guarantees |
| --- | --- | --- |
| `User::select()`, `TableOf::select()`, `User::find_where(…)` | `User` | The model of the table |
| `User::select_value(field)` | The field's type | The field belongs to the model and has its declared type |
| `User::select_fields((a, b))` | `(A, B)` | Every field belongs to the model, and the tuple holds their declared types in order (1 to 10 fields) |
| `select(…)`, `select_value(field)` | `Value` | Nothing |
| `.decode::<P>()` | `P` | Decoding only: a shape that doesn't match fails with [`Error::Conversion`](crate::Error::Conversion) |
| After `split`, `group_by`, `group_all` or `fetch` | `Value` again | These change the shape, so `decode` names the new one |
| After `fetch_link(path)` on `O = User` | `User` | The path is a chain of `User`'s links, so the fetched records decode into them |

`load_links` and `load_all_links` on a model `SELECT` keep the model type, since its links hold
either an id or a record. `fetch_link` also keeps it, over nested paths:
`Member::links().wallet.then(Wallet::links().bank)` fetches both hops.
`count_where` and `count_all` return a `SelectCount`, which reads `0` when there are no records.

**Clauses,** in the order SurrealDB parses them:
- `omit`;
- `from` or `from_only`;
- `with_index` or `with_no_index`;
- `where_`;
- `split`;
- `group_by` or `group_all`;
- `order_by` (the fields needn't be projected) or `order_rand`;
- `limit` and `start`;
- `fetch`;
- `version`;
- `timeout`;
- `for_update`;
- `tempfiles`;
- `explain` or `explain_full`.

[`explain`](crate::statements::explain) is the `EXPLAIN [ANALYZE] [FORMAT JSON]` prefix. It takes
a `SELECT` only, because SurrealDB refuses it over a write.

**Engine restrictions:**
- **`version`** needs a versioned store (SurrealKV with versioning). The memory engine refuses it.
- **`for_update`** takes no lock: a conflicting write makes the transaction's `COMMIT` fail.
  Rendering fails for a table, range or subquery target; with `VERSION`, `GROUP` or `SPLIT`; or
  with `LIMIT` over several targets.
- **`tempfiles`** takes effect only when the engine has a temporary directory. Otherwise it runs
  in memory, silently.

**Fields** are typed paths. `Field::new(name)` is one name, escaped when it isn't a plain
identifier or is a reserved keyword. `with_field`, indexes, filters, `optional()` (`.?`),
`flatten()` (`...`) and `destructure([…])` (`.{ … }`) extend a path. `Field::raw(text)` is the
one verbatim form, and a query that holds it can't bind values.

# Control flow and composition

- `let_(name)` writes `LET $name`, and `.typed::<V>()` adds the type.
- `if_`, `for_`, `break_`, `continue_`, `return_`, `throw` and `sleep` are statements.
- `block` and `chain`, and the `block!` and `chain!` macros, compose several statements.
- Reserved parameters are functions: `auth()`, `this()`, `parent()`, `access()`, `reference()`,
  `request()`, `action()`, `file()`, `target()`, and others. `$access`, `$auth`, `$token` and
  `$session` can't be set.
- `use_` changes the namespace or database for the rest of its own request only (see
  [`Executor`](crate::Executor)).

**Query transactions** compose statements into one request: `begin_transaction()`, then
`.query(…)` for each statement, then `.commit_transaction()` or `.cancel_transaction()`. The
`transaction!` macro does the same. The transaction opens and closes within that one request. A
transaction that spans several requests is an SDK [`Transaction`](crate::Transaction); see
[`Executor`](crate::Executor).

# Schema definitions

`define_*` builders cover:
- namespaces and databases;
- tables, fields, indexes and events;
- analyzers, functions and params;
- users and accesses;
- buckets, sequences, APIs, configs and modules.

`alter_*` and `remove_*` cover the same kinds wherever SurrealDB supports them. `rebuild_index`,
`info_for` and `show_changes_for_table`/`show_changes_for_database` complete the set.

- **Common options.** Every `define_*` takes `.overwrite()`, `.if_not_exists()` and
  `.comment(…)`. The last of `overwrite`/`if_not_exists` wins; on an existing resource,
  `IF NOT EXISTS` keeps it and returns `NONE`. Every `remove_*` takes `.if_exists()`.
- **Names.** Tables, fields, events, indexes, namespaces, databases, analyzers, users and
  accesses take a `Param` as their name. Functions and params don't, since SurrealDB takes only a
  literal name for them.
- **Tables.** `kind_any`, `kind_normal`, and
  `kind_relation(table_relation().in_(…).out(…).enforced())`. A table defined without `TYPE` is
  stored as `ANY`, and `SCHEMAFULL` alone as `NORMAL`.
- **Fields.** `default_always`, `readonly`, `flexible` (after a type that admits an object), and
  `computed`. Repeated `assert`s are joined with `AND`.
- **Events.** An optional `when`, repeated `then`, and `async_().retry(n).max_depth(n)`.
- **ALTER.** Each clause can be set or dropped (`drop_…`); setting and dropping the same clause
  is a render error.
- **INFO.** `info_for().root()`, `.namespace()`, `.database()` and `.table(t)` execute to typed
  results. Plain results map each name to its `DEFINE` text; `.structure()` returns objects.
