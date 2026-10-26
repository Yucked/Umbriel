Declares a model: a struct stored in SurrealDB, with a typed schema for building queries.

```text
#[umbriel::model(node, table = "user")]     a record in its own table
#[umbriel::model(edge, table = "follows")]  a record relating two nodes
#[umbriel::model(object)]                   a value embedded in another model
```

The attribute derives the SDK's `SurrealValue` for the struct, so values convert to and from the
database without Serde, and generates:

- for every model, its schema type `<Model>Schema`, returned by
  `<Model as SchemaGetter>::schema()`, whose fields are paths for queries
  (`User::schema().name`) and whose link methods traverse records and graph edges;
- for nodes and edges, the [`Model`] implementation (table, record id, field definitions for
  migrations) and `<Model>Patch`, a partial update with one [`Patch`] per stored field;
- for every model, `<Model>Merge`, a recursive `MERGE` with one [`Merge`] per stored field: an
  embedded object's field takes `Merge::Fields(<Object>Merge)` to merge only some of its fields,
  and a flattened object's field is a [`FlatMerge`];
- for objects, the [`Object`] implementation a model needs to flatten them.

# Record ids

Nodes and edges carry their id as `id: RecordId<Self>`. The key is always chosen by the caller:
`RecordId::rand()`, `RecordId::ulid()`, `RecordId::uuid()` or `User::id(key)`. Decoding checks the
table, so a `RecordId<User>` never holds a `post:…` id. Embedded objects have no id.

# Field names

Stored names come from the same attributes the `SurrealValue` derive reads:
`#[surreal(rename_all = "camelCase")]` on the struct and `#[surreal(rename = "…")]` on a field.
Schema fields are named after the stored name when it is a Rust identifier, and after the Rust
field otherwise. Two fields stored under one name fail to compile.

# Flattened objects

`#[surreal(flatten)]` on a field whose type is a `#[model(object)]` stores the object's fields as
the model's own:

- the field's schema is the object's schema at the model's path (`user.street`, not
  `user.address.street`);
- the object's field definitions are the model's;
- in `<Model>Patch` the field is a [`FlatPatch`]: `Keep` leaves every field of the object
  unchanged, and `Set(object)` assigns each of them, removing (`NONE`) any the value omits. There
  is no `None` or `Null`, since the object has no field of its own to remove or set to `NULL`.

Only objects can be flattened, since their stored names are known when compiling: maps and other
values are rejected, as are objects that store no field and object types that depend on the
model's generic parameters. A flattened field takes no `#[orm(…)]` settings, and `object!` does not
accept a model with one; use `content`, a patch or individual setters instead.

# Generated items

Everything is generated next to the model, outside any module, so a model can be declared in a
module or inside a function (as in the example below). Besides `<Model>Patch` and `<Model>Schema`,
nodes get `<Model>Aliases`, and hidden items named `__<Model>…` hold the schema's field and edge
types. Those names are taken: a type of your own named `UserSchema` or `UserPatch` next to `User`
does not compile.

# Example

```rust
use umbriel::{LinkOne, Model, Patch, RecordId, Render, SchemaGetter, model};

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

let alice = User {
    id: User::id("alice"),
    display_name: "Alice".to_owned(),
    age: 30,
    nickname: None,
    team: LinkOne::from(Team::id("acme")),
};

// Schema fields are paths named after the stored fields.
assert_eq!(User::schema().displayName.to_debug_sql(), "displayName");

// Only `age` changes; `nickname` is removed (`NONE`); every other field is kept.
let patch = UserPatch {
    age: Patch::Set(31),
    nickname: Patch::None,
    ..Default::default()
};
```

# Settings

The attribute itself takes the model kind, `table = "…"` for nodes and edges, and
`value = manual` to keep a hand-written `SurrealValue` implementation instead of deriving one.

Database-only settings stay in `#[orm(…)]`:

| On the struct | Meaning |
| --- | --- |
| `relax_table` | Allows a table name other than the struct name in snake case. |
| `schemafull`, `drop` | Flags of the generated `DEFINE TABLE`. |
| `as_ = …` | A select statement making the table a view. |
| `permissions = …` | `full`, `none`, or an expression giving the permissions. |
| `define = …` | An expression giving the whole `DEFINE TABLE`; excludes the settings above. |

| On a field | Meaning |
| --- | --- |
| `ty = "…"` | The stored type, written as in SurrealQL (`option<array<record<user>>>`); inferred from the Rust type when omitted. On `id` it is the record key's type (`int`, `string`, `uuid`, …); without it, `id` is not defined and takes any key. |
| `link_one`, `link_self`, `link_many` | The node a record link points to. |
| `nest_object`, `nest_array` | The embedded object type of the field or of its elements. |
| `relate(model = …, connection = "->edge->node")` | A read-only relation reached through an edge; never stored. |
| `value = …` | The field's default: `DEFAULT …` in the generated `DEFINE FIELD`, applied when a record is created without the field. Values written explicitly, falsy ones (`0`, `false`, `''`) included, are kept, and updates never re-apply it. |
| `assert`, `permissions`, `item_assert` | Clauses of the generated `DEFINE FIELD`. |
| `define = …` | An expression giving the whole `DEFINE FIELD`; excludes the clauses above. |
| `old_name = "…"` | The field's previous name, for migrations that rename it. |
