<div align="center">

# ☄️ Umbriel

**Type-safe SurrealDB for Rust.**

Expressive queries, ORM models, migrations, schema tooling and graph relations, all checked at
compile time.

![Rust 1.100+](https://img.shields.io/badge/rust-1.100%2B-f74c00?style=flat-square&logo=rust&logoColor=white)
![Edition 2024](https://img.shields.io/badge/edition-2024-f74c00?style=flat-square)
![SurrealDB 3.3.4+](https://img.shields.io/badge/SurrealDB-3.3.4%2B-ff00a0?style=flat-square&logo=surrealdb&logoColor=white)
![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success?style=flat-square)
![Status: pre-release](https://img.shields.io/badge/status-pre--release-lightgrey?style=flat-square)

[Quick start](#-quick-start) · [Models](#-models) · [Queries](#-queries) · [Migrations](#-migrations) · [Docs](#-documentation)

</div>

---

```rust
#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
    pub age: u8,
}

let UserSchema { age, .. } = User::schema();

let adults: Vec<User> = User::find_where(age.greater_than_or_equal(18))
    .order_by(order(&age).desc())
    .limit(10)
    .execute(&db)
    .await?;
```

<table>
<tr>
<td width="50%" valign="top">

### 🧬 Typed models
`#[model]` turns a struct into a node, an edge, a view or an embedded object, and generates a
schema, patch and merge type for it.

</td>
<td width="50%" valign="top">

### 🔍 Checked queries
Fields are typed paths. Functions and casts carry their SurrealQL kind. The return mode you pick
sets the Rust type you get back.

</td>
</tr>
<tr>
<td valign="top">

### 🕸️ Graph first
Traverse edges straight from the schema:
`Student::with(id).writes__(E).book(E)`.

</td>
<td valign="top">

### 🚚 Migrations
Diffs your models against your migration files and generates reversible migrations. Run them
from the CLI or compile them into your binary.

</td>
</tr>
</table>

> [!NOTE]
> Umbriel is a rebuild of [surreal_orm](https://github.com/Oyelowo/surreal_orm) for the current
> SurrealDB SDK. It is pre-release (`0.1.0`), not on crates.io yet, and its API is still
> changing.

## 🚀 Quick start

```toml
[dependencies]
umbriel = { git = "https://github.com/Yucked/Umbriel" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
let db = umbriel::engine::any::connect("ws://localhost:8000").await?;
db.use_ns("app").use_db("app").await?;
```

You only need `umbriel`. It re-exports the SDK's client, engines and options (`Surreal`,
`engine`, `opt`) and its value types (`types`). For an in-memory database, enable `kv-mem` and
connect to `mem://`.

Every call borrows an executor: the client (`&db`), or a transaction from
`db.clone().begin().await?`.

<details>
<summary><b>Feature flags</b></summary>

<br>

| Feature | Enables |
| --- | --- |
| `protocol-ws`, `rustls` | WebSocket transport with rustls TLS. **On by default.** |
| `protocol-http`, `protocol-grpc` | HTTP and gRPC transports. |
| `native-tls` | The platform's TLS instead of rustls. |
| `kv-mem`, `kv-rocksdb`, `kv-surrealkv`, `kv-indxdb`, `kv-tikv` | Embedded storage engines. |
| `scripting`, `http`, `jwks`, `ml`, `cjk` | SDK capabilities for embedded engines. |
| `parse` | Offline SurrealQL value parsing. |
| `migrations` | Schema migrations: generating, applying and rolling back, from files or embedded. |
| `cli` | The migration command line (`Migrator`); implies `migrations`. |

These pass through to the SurrealDB SDK feature of the same name.

</details>

## 🧬 Models

```rust
use umbriel::*;

#[model(node, table = "user")]
#[surreal(rename_all = "camelCase")]
pub struct User {
    pub id: RecordId<Self>,
    pub display_name: String,
    pub age: u8,
    pub nickname: Option<String>,

    #[orm(link_one = "Team")]
    pub team: LinkOne<Team>,
}

#[model(node, table = "team")]
pub struct Team {
    pub id: RecordId<Self>,
    pub name: String,
}
```

| Kind | Declared with | Stored as |
| --- | --- | --- |
| Node | `#[model(node, table = "…")]` | A record in its own table. |
| Edge | `#[model(edge, table = "…")]` | A record that relates two nodes. |
| View | `#[model(view, table = "…")]` | A read-only table computed by a query. |
| Object | `#[model(object)]` | A value embedded in another model. |

The stored type of each field comes from its Rust type, or from `#[orm(ty = "…")]`. The
[model guide](crates/orm/src/docs/model.md) lists every mapping and setting.

### CRUD

```rust
let alice = User {
    id: User::id("alice"),
    display_name: "Alice".to_owned(),
    age: 30,
    nickname: None,
    team: LinkOne::from(Team::id("acme")),
}
.create(&db)
.await?;

let found: Option<User> = User::get(&db, &alice.id).await?;

// Changes `age` and leaves every other field as it is.
User::patch(&db, &alice.id, UserPatch { age: Patch::Set(31), ..Default::default() }).await?;

User::delete_by_id(&db, &alice.id).await?;
```

There are also `save` (upsert), `insert_many`, `get_many`, `get_range`, `merge` and `delete`.

## 🔍 Queries

Each SurrealQL statement has a typed builder in `umbriel::statements`. The return mode you pick
sets both the `RETURN` clause and the Rust type you get back.

| | Builders |
| --- | --- |
| **Read** | select, live select |
| **Write** | create, insert, update, upsert, relate, delete |
| **Schema** | define and remove, for tables, fields, indexes, events, functions and more |
| **Control flow** | let, if / else, for, return, throw |

When a builder isn't enough, these macros let you write the query as Rust code or as raw
SurrealQL:

| Macro | Use it for |
| --- | --- |
| `query!` | Raw SurrealQL with named bindings, one query or several |
| `chain!`, `block!`, `transaction!` | Multi-statement chains, blocks and transactions |
| `for_!` | A loop over values |
| `define_function!` | A custom function and typed helpers to call it |

```rust
let mut response = query!(db, "RETURN $name", { name: "Alice" }).await?;
let name: Option<String> = response.take(0)?;
```

## 🚚 Migrations

Enable the `cli` feature, list your models, and hand them to the CLI:

```rust
use umbriel::{DbResources, TableResources, create_table_resources, migrations::Migrator};

// Each listed model implements `TableResources`: empty, or with its events and indexes.
impl TableResources for User {}
impl TableResources for Team {}

struct Resources;

impl DbResources for Resources {
    create_table_resources!(User, Team);
}

#[tokio::main]
async fn main() -> umbriel::migrations::MigrationResult<()> {
    Migrator::run(Resources).await
}
```

```sh
cargo run -- init --name initial -r   # first migration; -r makes it reversible
cargo run -- gen  --name add_teams    # diff the models against the migrations
cargo run -- up                        # apply pending   (-n N, --till <file>)
cargo run -- down                      # roll back       (-n N, --till <file>, --prune)
cargo run -- list --status pending     # all | pending | applied
cargo run -- prune                     # delete unapplied files, after asking (--yes skips)
```

<details>
<summary><b>Connection flags and embedded migrations</b></summary>

<br>

The connection flags are `--url` (default `ws://localhost:8000`), `--ns`, `--db`, `--user` and
`--pass`. The user and password have no default; they can also come from `UMBRIEL_USER` and
`UMBRIEL_PASS`, which keeps the password out of the process list. Use `--dir` for a custom
migrations directory. The generator compares schemas in
temporary databases, created in the namespace you give with `--comparison-ns`. Run
`cargo run -- help` for the full list.

To compile migrations into your binary and apply them at startup:

```rust
use umbriel::migrations::{EmbeddedMigrationsOneWay, Mode, UpdateStrategy, embed_migrations};

const MIGRATIONS: EmbeddedMigrationsOneWay = embed_migrations!("migrations");

MIGRATIONS.run(&db, UpdateStrategy::Latest, Mode::Strict).await?;
```

</details>

## 📚 Documentation

| | |
| --- | --- |
| [Consumer API](docs/api.md) | Call forms, return modes, execution and errors. |
| [Model guide](crates/orm/src/docs/model.md) | `#[model]` settings, stored types, record ids and table families. |
| [Feature inventory](docs/feature-inventory.md) | What is supported, and the evidence for it. |
| `cargo doc -p umbriel --open` | The API reference. |

## 🛠️ Development

```sh
cargo build  --workspace --all-targets
cargo clippy --workspace --all-targets
cargo test   --workspace
```

| Crate | Role |
| --- | --- |
| [`umbriel`](crates/orm) | Models, queries, schema and execution. Migrations sit behind a feature. |
| [`umbriel-macros`](crates/macros) | The derives and query macros, re-exported by `umbriel`. |
| [`umbriel-tests`](crates/tests) | Every test. They run against SurrealDB servers you start; see its [README](crates/tests/README.md). |

The contribution rules are in [AGENTS.md](AGENTS.md).

---

<div align="center">
<sub>Built on <a href="https://github.com/Oyelowo/surreal_orm">surreal_orm</a> by Oyelowo Oyedayo. See <a href="LICENSE">LICENSE</a> for attribution.</sub>
</div>
