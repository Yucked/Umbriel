

# Classification

Statements, model methods and SDK calls share one classification. It reads only the SDK error's
structured details, never its message:

| Error | When |
| --- | --- |
| `RecordExists { id, .. }` | A record with the id already exists |
| `TableNotFound { table, .. }` | The table isn't defined, for example on a `STRICT` database |
| `Conflict { .. }` | The transaction conflicted with another and committed nothing |
| `NotAllowed { .. }` | A permission, authentication or capability refusal |
| `Connection { .. }` | The client was never connected, is already connected, or its transport failed |
| `Statement { .. }` / `Query(..)` | Anything else |

Each variant keeps the SDK error as its source. When it came from a statement of a query, it
also keeps the statement's index and the response's other statement errors (`statement` or
`related`). `From<surrealdb::Error>` applies the same classification, so the errors of SDK calls
such as `begin`, `commit` and `cancel` convert with `?`.

A unique index conflict has no structured kind in SurrealDB 3.3.4, so it stays a `Statement`
error.

# Retrying

[`Error::retry`] is advice for the caller's own retry policy; Umbriel retries nothing itself.

| [`Retry`] | Errors | Meaning |
| --- | --- | --- |
| `Safe` | `Conflict` | Restart the failed transaction. Writes committed before it, such as earlier statements of the same batch outside it, stay applied, so replaying a larger batch isn't safe |
| `Uncertain` | A failed transport, a server-reported timeout, a cancellation, a commit whose outcome is unknown, and any SDK error without a structured kind | There isn't enough information to tell whether the write applied: check the database before writing again |
| `Never` | Failures before anything is sent (`QueryBuilder`, a model's table and field checks, a client never connected or already connected); server refusals (`RecordExists`, `TableNotFound`, `NotAllowed`, and SDK validation, thrown, not-found, already-exists and configuration errors, and statements that weren't executed); failures after a finished run (`Conversion`, `TooManyItemsReturned`, `RecordNotFound`) | No retry is recommended. This isn't a guarantee that nothing applied |

A migration failure takes the advice of the database error it wraps.

```rust
use umbriel::{Error, Retry};

/// Whether a failed transaction can simply be run again from its beginning.
fn can_restart(error: &Error) -> bool {
    error.retry() == Retry::Safe
}

assert!(!can_restart(&Error::QueryBuilder("no table".to_owned())));
```

**Engine difference.** The advice depends on the structured kind the engine reports. The
embedded engine loses it for an SDK transaction handle's `commit`, so a conflict there is
`Query` with `Uncertain`, not `Conflict` with `Safe`. Over WebSocket it is `Conflict` with `Safe`.
A query transaction's conflicting `COMMIT` keeps its kind on every engine.
