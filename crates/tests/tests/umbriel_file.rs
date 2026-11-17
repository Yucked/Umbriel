//! `file::*` (`orm/src/functions/file.rs`): every function and method form rendered, and a
//! file's lifecycle in a `memory` bucket on a database that allows the experimental `files`
//! feature.

use surrealdb::{
    Surreal,
    engine::any::Any,
    types::{Bytes, File, Number, Object, SurrealValue, Value},
};
use umbriel::{
    FileLike, Function, Render, Runnable, functions::file, kinds::KindOf, statements::define_bucket,
};
use umbriel_tests::*;

fn alice() -> FileLike {
    FileLike::from(File::new("avatars", "alice.txt"))
}

fn bob() -> FileLike {
    FileLike::from(File::new("avatars", "bob.txt"))
}

/// `file::list` options listing the keys that start with `prefix`.
fn prefixed(prefix: &str) -> Object {
    let mut options = Object::new();
    options.insert("prefix", prefix.to_owned());
    options
}

#[test]
fn rendering() {
    calls!(
        (file::bucket!(alice()), "file::bucket(f\"avatars:/alice.txt\")"),
        (file::key!(alice()), "file::key(f\"avatars:/alice.txt\")"),
        (
            file::put!(alice(), "Alice"),
            "file::put(f\"avatars:/alice.txt\", 'Alice')"
        ),
        (
            file::put_if_not_exists!(alice(), "Alice"),
            "file::put_if_not_exists(f\"avatars:/alice.txt\", 'Alice')"
        ),
        (file::get!(alice()), "file::get(f\"avatars:/alice.txt\")"),
        (file::head!(alice()), "file::head(f\"avatars:/alice.txt\")"),
        (file::delete!(alice()), "file::delete(f\"avatars:/alice.txt\")"),
        (
            file::copy!(alice(), "bob.txt"),
            "file::copy(f\"avatars:/alice.txt\", 'bob.txt')"
        ),
        (
            file::copy_if_not_exists!(alice(), bob()),
            "file::copy_if_not_exists(f\"avatars:/alice.txt\", f\"avatars:/bob.txt\")"
        ),
        (
            file::rename!(alice(), "bob.txt"),
            "file::rename(f\"avatars:/alice.txt\", 'bob.txt')"
        ),
        (
            file::rename_if_not_exists!(alice(), "bob.txt"),
            "file::rename_if_not_exists(f\"avatars:/alice.txt\", 'bob.txt')"
        ),
        (file::exists!(alice()), "file::exists(f\"avatars:/alice.txt\")"),
        (file::list!("avatars"), "file::list('avatars')"),
        (
            file::list!("avatars", prefixed("a")),
            "file::list('avatars', { prefix: 'a' })"
        ),
        (alice().exists(), "(f\"avatars:/alice.txt\").exists()"),
        (alice().put("Alice"), "(f\"avatars:/alice.txt\").put('Alice')"),
        (alice().rename("bob.txt"), "(f\"avatars:/alice.txt\").rename('bob.txt')"),
    );
}

/// Runs `function` with its values bound, as an application does.
async fn run<T: SurrealValue, K: KindOf>(
    db: &Surreal<Any>,
    function: Function<K>,
) -> Option<T> {
    function.run(db).await.expect("ran")
}

/// A database with the `memory` bucket `avatars`.
async fn avatars() -> Surreal<Any> {
    let db = experimental_db().await;
    define_bucket("avatars")
        .backend("memory")
        .run(&db)
        .await
        .expect("bucket defined");
    db
}

#[tokio::test]
async fn a_file_is_written_read_copied_renamed_listed_and_deleted() {
    let db = avatars().await;
    assert_eq!(run::<bool, _>(&db, file::exists!(alice())).await, Some(false));
    let missing = run::<Value, _>(&db, file::get!(alice())).await;
    assert!(matches!(missing, None | Some(Value::None)), "{missing:?}");

    run::<Value, _>(&db, alice().put("Alice")).await;
    assert_eq!(run::<bool, _>(&db, alice().exists()).await, Some(true));
    let contents = run::<Bytes, _>(&db, file::get!(alice()))
        .await
        .expect("the file is read")
        .into_inner();
    assert_eq!(contents.as_ref(), b"Alice");
    let head: Object = run(&db, file::head!(alice()))
        .await
        .expect("the file has metadata");
    assert_eq!(head.get("size"), Some(&Value::Number(Number::Int(5))));
    assert_eq!(
        run::<String, _>(&db, file::key!(alice())).await.as_deref(),
        Some("/alice.txt")
    );
    assert_eq!(
        run::<String, _>(&db, alice().bucket()).await.as_deref(),
        Some("avatars")
    );

    run::<Value, _>(&db, file::copy!(alice(), "bob.txt")).await;
    let listed: Vec<Object> = run(&db, file::list!("avatars"))
        .await
        .expect("the bucket is listed");
    assert_eq!(listed.len(), 2, "{listed:?}");
    let limited: Vec<Object> = run(&db, file::list!("avatars", prefixed("b")))
        .await
        .expect("the bucket is listed");
    assert_eq!(limited.len(), 1, "{limited:?}");

    run::<Value, _>(&db, file::delete!(bob())).await;
    run::<Value, _>(&db, file::rename!(alice(), "bob.txt")).await;
    assert_eq!(run::<bool, _>(&db, file::exists!(alice())).await, Some(false));
    assert_eq!(run::<bool, _>(&db, file::exists!(bob())).await, Some(true));
}

#[tokio::test]
async fn the_if_not_exists_forms_fail_on_an_existing_file() {
    let db = avatars().await;
    run::<Value, _>(&db, file::put!(alice(), "Alice")).await;
    run::<Value, _>(&db, file::put!(bob(), "Bob")).await;
    for refused in [
        file::put_if_not_exists!(alice(), "Alicia"),
        file::copy_if_not_exists!(alice(), "bob.txt"),
        file::rename_if_not_exists!(alice(), "bob.txt"),
    ] {
        let result: umbriel::Result<Option<Value>> = refused.run(&db).await;
        assert!(result.is_err(), "{}", refused.to_debug_sql());
    }
    let contents = run::<Bytes, _>(&db, file::get!(bob()))
        .await
        .expect("the file is read")
        .into_inner();
    assert_eq!(contents.as_ref(), b"Bob", "the target is unchanged");
}

#[tokio::test]
async fn a_read_only_bucket_refuses_writes() {
    let db = experimental_db().await;
    define_bucket("avatars")
        .backend("memory")
        .readonly()
        .run(&db)
        .await
        .expect("bucket defined");
    let result: umbriel::Result<Option<Value>> = file::put!(alice(), "Alice").run(&db).await;
    assert!(result.is_err(), "{result:?}");
}

#[tokio::test]
async fn files_need_the_files_feature() {
    let db = db().await;
    let error = call_failure(&db, &file::exists!(alice())).await;
    assert!(error.to_lowercase().contains("files"), "{error}");
}
