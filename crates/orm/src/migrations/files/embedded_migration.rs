use std::marker::PhantomData;

use surrealdb::{Surreal, engine::any::Any};

use crate::migrations::{
    FileMetadata, MigrationConfig, MigrationError, MigrationFileOneWay, MigrationFileTwoWayPair,
    MigrationFilename, MigrationResult, Mode, UpdateStrategy,
};

/// Marker for migrations stored as single `.surql` files.
#[derive(Clone, Copy, Debug)]
pub struct OneWay;

/// Marker for migrations stored as `.up.surql`/`.down.surql` pairs.
#[derive(Clone, Copy, Debug)]
pub struct TwoWay;

/// Migration files embedded by `embed_migrations!`.
///
/// The macro only lists and includes the files; names are parsed and paired here, so the naming
/// rules have a single implementation. The direction is chosen by the declared type.
#[derive(Debug)]
pub struct EmbeddedMigrations<D> {
    files: &'static [FileMetadataStatic],
    direction: PhantomData<D>,
}

impl<D> Clone for EmbeddedMigrations<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for EmbeddedMigrations<D> {}

impl<D> EmbeddedMigrations<D> {
    /// Wraps embedded files listed in file name order.
    pub const fn new(files: &'static [FileMetadataStatic]) -> Self {
        Self {
            files,
            direction: PhantomData,
        }
    }

    /// Embedded files in filename order.
    pub const fn files(&self) -> &'static [FileMetadataStatic] {
        self.files
    }

    fn file_metadata(file: &FileMetadataStatic) -> MigrationResult<FileMetadata> {
        let name = MigrationFilename::try_from(file.name.to_string())?;
        Ok(FileMetadata::new(name, file.content.to_string().into()))
    }
}

/// Embedded one-way migrations.
pub type EmbeddedMigrationsOneWay = EmbeddedMigrations<OneWay>;
/// Embedded two-way migrations.
pub type EmbeddedMigrationsTwoWay = EmbeddedMigrations<TwoWay>;

impl EmbeddedMigrationsOneWay {
    /// Applies the pending embedded migrations as far as `update_strategy` says, each in its own
    /// transaction (see [`MigrationRunner`](crate::migrations::MigrationRunner)).
    pub async fn run(
        self,
        db: &Surreal<Any>,
        update_strategy: UpdateStrategy,
        mode: Mode,
    ) -> MigrationResult<()> {
        let files_config = MigrationConfig::new().set_mode(mode);
        files_config
            .one_way()
            .run_embedded_pending_migrations(db, self, update_strategy)
            .await?;
        Ok(())
    }

    /// The embedded files as one-way migrations; a two-way file name is an error.
    pub fn to_migrations_one_way(&self) -> MigrationResult<Vec<MigrationFileOneWay>> {
        self.files
            .iter()
            .map(|file| {
                let meta = Self::file_metadata(file)?;
                if !meta.name.is_unidirectional() {
                    return Err(MigrationError::InvalidMigrationFileNameForMode(
                        file.name.to_string(),
                    ));
                }
                Ok(MigrationFileOneWay::new(meta))
            })
            .collect()
    }
}

impl EmbeddedMigrationsTwoWay {
    /// Applies the pending embedded migrations as far as `update_strategy` says, each in its own
    /// transaction (see [`MigrationRunner`](crate::migrations::MigrationRunner)).
    pub async fn run(
        self,
        db: &Surreal<Any>,
        update_strategy: UpdateStrategy,
        mode: Mode,
    ) -> MigrationResult<()> {
        let files_config = MigrationConfig::new().set_mode(mode);

        let two_way = files_config.two_way();
        two_way
            .run_up_embedded_pending_migrations(db, self, update_strategy)
            .await?;
        Ok(())
    }

    /// Pairs each `.up.surql` file with its `.down.surql` counterpart; a file without its
    /// counterpart in either direction is an error.
    pub fn to_migrations_two_way(&self) -> MigrationResult<Vec<MigrationFileTwoWayPair>> {
        let files = self
            .files
            .iter()
            .map(Self::file_metadata)
            .collect::<MigrationResult<Vec<_>>>()?;

        if let Some(invalid) = files.iter().find(|file| file.name.is_unidirectional()) {
            return Err(MigrationError::InvalidMigrationFileNameForMode(
                invalid.name.to_string(),
            ));
        }

        let counterpart = |name: &MigrationFilename| {
            let expected = if name.is_up() {
                name.to_down()
            } else {
                name.to_up()
            };
            files
                .iter()
                .find(|file| file.name == expected)
                .ok_or_else(|| MigrationError::MigrationFileDoesNotExist(expected.to_string()))
        };

        let mut pairs = Vec::with_capacity(files.len() / 2);
        for file in &files {
            let other = counterpart(&file.name)?;
            if file.name.is_up() {
                pairs.push(MigrationFileTwoWayPair::new(file.clone(), other.clone()));
            }
        }
        Ok(pairs)
    }
}

/// An embedded migration file.
#[derive(Clone, Debug)]
pub struct FileMetadataStatic {
    /// The file's name.
    pub name: &'static str,
    /// The file's content.
    pub content: &'static str,
}
