use std::ops::Deref;

use surrealdb::Connection;

use crate::{DbResources, Raw, Runnable, statements::begin_transaction};

use crate::migrations::*;

/// The comparison database holding the codebase's schema, the right side of a diff.
#[derive(Debug, Clone)]
pub struct RightDatabase<C: Connection>(
    /// The comparison database.
    pub MigratorDatabase<C>,
);

impl<C: Connection> RightDatabase<C> {
    /// Applies the migration history table and the codebase's definitions in one transaction.
    pub async fn run_codebase_schema_queries(
        &self,
        code_resources: &impl DbResources,
        migration_type: MigrationFlag,
    ) -> MigrationResult<()> {
        let mut transaction = begin_transaction().query(Migration::define_table());
        for definition in Migration::define_fields(migration_type)
            .into_iter()
            .chain(Self::get_codebase_schema_queries(code_resources))
        {
            transaction = transaction.query(definition);
        }
        transaction.commit_transaction().run(self.db()).await?;
        Ok(())
    }

    /// The codebase's definitions in creation order (see [`ResourceKind`]), so each one's
    /// prerequisites exist when it runs: a module may load from a bucket, an analyzer may use a
    /// function, an index an analyzer, and an API a function or table.
    pub fn get_codebase_schema_queries(db_resources: &impl DbResources) -> Vec<Raw> {
        [
            db_resources.buckets(),
            db_resources.modules(),
            db_resources.functions(),
            db_resources.analyzers(),
            db_resources.params(),
            db_resources.sequences(),
            db_resources.accesses(),
            db_resources.users(),
            db_resources.tables(),
            db_resources.apis(),
            db_resources.configs(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl<C: Connection> Deref for RightDatabase<C> {
    type Target = MigratorDatabase<C>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
