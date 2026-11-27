//! Patches (`orm/src/query/patch.rs`): partial updates of a model, keeping `NONE` and `NULL`
//! apart.

use umbriel_tests::{support::models::*, *};

#[test]
fn kept_fields_are_omitted_and_none_null_and_values_are_distinct() {
    let patch = ProfilePatch {
        name: Patch::Keep,
        nickname: Patch::None,
        bio: Patch::Null,
        ..Default::default()
    };
    assert_eq!(
        patch.into_setters().to_debug_sql(),
        "nickname = NONE, bio = NULL"
    );

    let patch = ProfilePatch {
        bio: Patch::Set(Some(Nullable::Value("Hello".to_owned()))),
        ..Default::default()
    };
    assert_eq!(patch.into_setters().to_debug_sql(), "bio = 'Hello'");

    assert!(ProfilePatch::default().into_setters().is_empty());
    assert!(matches!(ProfilePatch::default().bio, Patch::Keep));
}

#[tokio::test]
async fn patch_updates_only_the_given_fields() {
    let (db, _guard) = db().await;
    profile("alice", Some("Al"), None)
        .create(&db)
        .await
        .expect("created");

    let patched = Profile::patch(
        &db,
        &Profile::id("alice"),
        ProfilePatch {
            name: Patch::Set("Alicia".to_owned()),
            nickname: Patch::None,
            ..Default::default()
        },
    )
    .await
    .expect("patched")
    .expect("the record exists");
    assert_eq!(patched.name, "Alicia");
    assert_eq!(patched.nickname, None);
    assert_eq!(patched.scores, HashSet::from([1, 2]));
}

#[tokio::test]
async fn patch_keeps_none_and_null_distinct() {
    let (db, _guard) = db().await;
    profile("alice", None, Some(Nullable::Value("Hello".to_owned())))
        .create(&db)
        .await
        .expect("created");

    let nulled = Profile::patch(
        &db,
        &Profile::id("alice"),
        ProfilePatch {
            bio: Patch::Null,
            ..Default::default()
        },
    )
    .await
    .expect("patched")
    .expect("exists");
    assert_eq!(nulled.bio, Some(Nullable::Null));
    let is_null: bool = read(&db, "RETURN type::is_null(profile:alice.bio);").await;
    assert!(is_null);

    let removed = Profile::patch(
        &db,
        &Profile::id("alice"),
        ProfilePatch {
            bio: Patch::None,
            ..Default::default()
        },
    )
    .await
    .expect("patched")
    .expect("exists");
    assert_eq!(removed.bio, None);
    let is_none: bool = read(&db, "RETURN type::is_none(profile:alice.bio);").await;
    assert!(is_none);
}

#[tokio::test]
async fn setting_a_nested_object_replaces_it_whole() {
    let (db, _guard) = db().await;
    let id = Marker::id("origin");
    Marker {
        id: id.clone(),
        offsets: Offsets {
            x: Some(1),
            y: Some(2),
        },
    }
    .create(&db)
    .await
    .expect("created");
    let stored = || read::<BTreeMap<String, i64>>(&db, "RETURN marker:origin.offsets;");

    let set = |offsets: Offsets| MarkerPatch {
        offsets: Patch::Set(offsets),
        ..Default::default()
    };
    Marker::patch(
        &db,
        &id,
        set(Offsets {
            x: None,
            y: Some(3),
        }),
    )
    .await
    .expect("patched")
    .expect("the record exists");
    assert_eq!(stored().await, BTreeMap::from([("y".to_owned(), 3)]));

    Marker::patch(&db, &id, set(Offsets::default()))
        .await
        .expect("patched")
        .expect("the record exists");
    assert_eq!(stored().await, BTreeMap::new());
}

#[tokio::test]
async fn removing_a_field_brings_back_only_a_default_always() {
    // Typed `DEFAULT ALWAYS` support belongs to #13; the definitions are written by hand.
    for (default, expected) in [
        ("DEFAULT 'Anon'", None),
        ("DEFAULT ALWAYS 'Anon'", Some("Anon")),
    ] {
        let (db, _guard) = db().await;
        Raw::new(format!(
            "DEFINE TABLE profile SCHEMALESS; \
                 DEFINE FIELD nickname ON TABLE profile TYPE option<string> {default};"
        ))
        .run(&db)
        .await
        .expect("defined");
        profile("alice", Some("Al"), None)
            .create(&db)
            .await
            .expect("created");

        let patched = Profile::patch(
            &db,
            &Profile::id("alice"),
            ProfilePatch {
                nickname: Patch::None,
                ..Default::default()
            },
        )
        .await
        .expect("patched")
        .expect("the record exists");
        assert_eq!(patched.nickname.as_deref(), expected, "{default}");
    }
}

#[tokio::test]
async fn a_patch_that_keeps_everything_reads_the_record() {
    let (db, _guard) = db().await;
    profile("alice", Some("Al"), None)
        .create(&db)
        .await
        .expect("created");

    let read = Profile::patch(&db, &Profile::id("alice"), ProfilePatch::default())
        .await
        .expect("read")
        .expect("exists");
    assert_eq!(read.nickname.as_deref(), Some("Al"));
}

#[tokio::test]
async fn patching_a_missing_record_creates_nothing() {
    let (db, _guard) = db().await;
    // SurrealDB 3 reports reading a table that does not exist as an error.
    Profile::define_table()
        .run(&db)
        .await
        .expect("table defined");
    let missing = Profile::patch(
        &db,
        &Profile::id("nobody"),
        ProfilePatch {
            name: Patch::Set("Nobody".to_owned()),
            ..Default::default()
        },
    )
    .await
    .expect("ran");
    assert!(missing.is_none());
    assert!(
        Profile::get(&db, &Profile::id("nobody"))
            .await
            .expect("read")
            .is_none()
    );
}
