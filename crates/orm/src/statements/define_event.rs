use surrealdb::types::Table;

use crate::{Conditional, Event, Filter, Name, Renderer};

use super::{DefineOptions, Statement, Subquery, impl_define_options, impl_definition};

/// A DEFINE EVENT statement awaiting what it runs.
pub struct EventBuilder {
    event: Name<Event>,
    on_table: Option<Name<Table>>,
    when: Option<Filter>,
}

/**
Define a new event.
Events can be triggered after any change or modification to the data in a record. Each trigger is able to see the $before and $after value of the record, enabling advanced custom logic with each trigger.

Requirements
You must be authenticated as a root, namespace, or database user before you can use the DEFINE EVENT statement.
You must select your namespace and database before you can use the DEFINE EVENT statement.

# Example
```rust
use umbriel::{*, statements::{define_event, select}, types::RecordId};

# let age = Field::new("age");
# let city = Field::new("city");
# let fake_id = RecordId::new("user", "alice");
# let user_table = Table::new("user");
# let email_event = Event::new("email");

 let query = define_event(email_event)
    .on_table(user_table)
     .when(age.greater_than_or_equal(18))
     .then(
         select(All)
             .from(fake_id)
             .where_(
                 cond(city.is("Prince Edward Island"))
                     .and(city.is("NewFoundland"))
                     .or(city.is("Toronto")),
             )
             .limit(153)
             .start(10),
     );

 assert_eq!(
     query.to_debug_sql(),
     "DEFINE EVENT email ON TABLE user WHEN age >= 18 THEN (SELECT * FROM user:alice \
WHERE ((city IS 'Prince Edward Island') AND (city IS 'NewFoundland')) OR (city IS 'Toronto') \
LIMIT 153 START AT 10);",
);
```

`WHEN` is optional: without it the event runs on every change. Each further `then` adds a
statement, run in order:

```rust
use umbriel::{*, statements::{define_event, select}};

let query = define_event("audit")
    .on_table("user")
    .then(select(All).from(Table::new("log")))
    .then(select(All).from(Table::new("archive")))
    .async_()
    .retry(3)
    .max_depth(2);
assert_eq!(
    query.to_debug_sql(),
    "DEFINE EVENT audit ON TABLE user THEN (SELECT * FROM log), (SELECT * FROM archive) \
ASYNC RETRY 3 MAXDEPTH 2;",
);
```
*/
pub fn define_event(event_name: impl Into<Name<Event>>) -> EventBuilder {
    EventBuilder {
        event: event_name.into(),
        on_table: None,
        when: None,
    }
}

impl EventBuilder {
    /// Set the event table. The name may come from a parameter.
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.on_table = Some(table.into());
        self
    }

    /// Set the event trigger: the event runs only for changes where `condition` holds. Without
    /// one, it runs on every change.
    pub fn when(mut self, condition: impl Conditional) -> Self {
        self.when = Some(Filter::new(condition));
        self
    }

    /// What the event runs: a statement, rendered as a subquery, or a block.
    pub fn then(self, query: impl Into<Subquery>) -> DefineEventStatement {
        DefineEventStatement {
            event: self.event,
            on_table: self.on_table,
            options: DefineOptions::default(),
            when: self.when,
            then: vec![query.into()],
            mode: EventMode::default(),
        }
    }
}

/// Whether an event is `ASYNC`, how often a failed run is retried, and how deeply it may
/// trigger further events. Shared with `ALTER EVENT`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EventMode {
    pub(crate) asynchronous: bool,
    pub(crate) retry: Option<u16>,
    pub(crate) max_depth: Option<u16>,
}

impl EventMode {
    /// Writes ` ASYNC [RETRY n] [MAXDEPTH n]`. `RETRY` and `MAXDEPTH` only follow `ASYNC`, so
    /// either without it fails the query.
    pub(crate) fn render(self, out: &mut Renderer) {
        if !self.asynchronous {
            if self.retry.is_some() || self.max_depth.is_some() {
                out.push_error("RETRY and MAXDEPTH apply only to ASYNC events; call `async_`");
            }
            return;
        }
        out.push_str(" ASYNC");
        if let Some(retry) = self.retry {
            out.push_str(" RETRY ").push_display(&retry);
        }
        if let Some(max_depth) = self.max_depth {
            out.push_str(" MAXDEPTH ").push_display(&max_depth);
        }
    }
}

/// A statement for defining an event.
pub struct DefineEventStatement {
    event: Name<Event>,
    on_table: Option<Name<Table>>,
    options: DefineOptions,
    when: Option<Filter>,
    then: Vec<Subquery>,
    mode: EventMode,
}

impl DefineEventStatement {
    /// Adds a statement the event runs, after the earlier ones.
    pub fn then(mut self, query: impl Into<Subquery>) -> Self {
        self.then.push(query.into());
        self
    }

    /// `ASYNC`: the event runs after the triggering transaction commits, with at-least-once
    /// delivery, instead of inside it.
    pub fn async_(mut self) -> Self {
        self.mode.asynchronous = true;
        self
    }

    /// `RETRY n`: how many times a failed `ASYNC` event is retried. Without
    /// [`async_`](Self::async_) the query fails when it is rendered.
    pub fn retry(mut self, retries: u16) -> Self {
        self.mode.retry = Some(retries);
        self
    }

    /// `MAXDEPTH n`: how deeply an `ASYNC` event may trigger further events. Without
    /// [`async_`](Self::async_) the query fails when it is rendered.
    pub fn max_depth(mut self, depth: u16) -> Self {
        self.mode.max_depth = Some(depth);
        self
    }
}

impl Statement for DefineEventStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE EVENT");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.event);
        if let Some(table) = &self.on_table {
            out.push_str(" ON TABLE ").push(table);
        }
        if let Some(when) = &self.when {
            out.push_str(" WHEN ").push(when);
        }
        out.push_str(" THEN ").push_joined(&self.then, ", ");
        self.mode.render(out);
        self.options.render_comment(out);
    }
}

impl_define_options!(DefineEventStatement);
impl_definition!(DefineEventStatement);
