use super::up::{FastForwardDelta, Up};

use crate::DbResources;
use crate::migrations::*;
use clap::Args;
use typed_builder::TypedBuilder;

/// Generate migrations
#[derive(Args, Debug, TypedBuilder, Clone)]
pub struct Generate {
    /// Name of the migration
    #[arg(long)]
    pub(crate) name: Basename,

    /// Whether to run the migrations after generation
    #[arg(long)]
    #[builder(default)]
    pub(crate) run: bool,
}

impl Generate {
    /// Generates a migration from the difference between the migration directory and the
    /// codebase's resources, and applies it if asked.
    ///
    /// # Panics
    ///
    /// If the migration type cannot be detected or generation fails.
    pub async fn run(
        &self,
        cli: &mut Migrator,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) {
        let file_manager = cli.file_manager();
        let migration_basename = &self.name;
        let mig_type = file_manager.detect_migration_type();
        let comparison = cli.comparison().await;

        match mig_type {
            Ok(MigrationFlag::TwoWay) => {
                log::info!("Generating two-way migration");
                let generated = file_manager
                    .two_way()
                    .generate_migrations(
                        &comparison,
                        migration_basename,
                        codebase_resources,
                        prompter,
                    )
                    .await;
                if let Err(e) = generated {
                    log::error!("Failed to generate migrations: {e}");
                    panic!("Failed to generate migrations: {e}");
                }
            }
            Ok(MigrationFlag::OneWay) => {
                let generated = file_manager
                    .one_way()
                    .generate_migrations(
                        &comparison,
                        migration_basename,
                        codebase_resources,
                        prompter,
                    )
                    .await;

                if let Err(e) = generated {
                    log::error!("Failed to generate migrations: {e}");
                    panic!("Failed to generate migrations: {e}");
                }
            }
            Err(e) => {
                log::error!("Failed to detect migration type: {e}");
                panic!("Failed to detect migration type");
            }
        };

        if self.run {
            cli.setup_db().await;
            log::info!("Running generated migrations");

            self.up().run(cli).await;

            log::info!("Successfully ran the generated migration(s)");
        }

        log::info!("Migration generation done.");
    }

    fn up(&self) -> Up {
        Up {
            fast_forward: FastForwardDelta {
                latest: true,
                number: None,
                till: None,
            },
        }
    }
}
