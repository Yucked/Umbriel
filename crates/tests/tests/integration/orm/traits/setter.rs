//! Setters (`orm/src/traits/setter.rs`): assignments, numeric and array assignments, and patch
//! operations on model fields.

use umbriel_tests::{support::models::*, *};

#[test]
fn assignments_bind_values_and_write_expressions() {
    let org = Organization::schema();

    let query = org.age.equal_to(34u8).to_query().expect("valid");
    assert_eq!(query.sql(), "age = $_p0");
    assert_eq!(query.vars().len(), 1);
    assert_eq!(org.age.equal_to(34u8).to_debug_sql(), "age = 34");
    assert_eq!(
        org.age.equal_to(Field::new("age")).to_debug_sql(),
        "age = age"
    );

    let connected = org
        .time()
        .connected
        .equal_to(DateTime::<Utc>::UNIX_EPOCH)
        .to_query()
        .expect("valid");
    assert_eq!(connected.sql(), "time.connected = $_p0");
}

#[test]
fn numeric_and_array_assignments() {
    let org = Organization::schema();
    assert_eq!(org.age.increment_by(1u8).to_debug_sql(), "age += 1");
    assert_eq!(org.age.decrement_by(1u8).to_debug_sql(), "age -= 1");

    let planet = Planet::<u64>::schema();
    assert_eq!(planet.tags.append(45u64).to_debug_sql(), "tags += 45");
    assert_eq!(planet.tags.remove(45u64).to_debug_sql(), "tags -= 45");
    assert_eq!(
        org.users.append(User::id("alice")).to_debug_sql(),
        "users += user:alice"
    );
}

#[test]
fn patch_operations_use_json_pointers() {
    let org = Organization::schema();
    assert_eq!(
        org.name.patch_add("Acme Corp").to_debug_sql(),
        "{ op: 'add', path: '/name', value: 'Acme Corp' }"
    );
    assert_eq!(
        org.name.patch_replace("Globex").to_debug_sql(),
        "{ op: 'replace', path: '/name', value: 'Globex' }"
    );
    assert_eq!(
        org.time().connected.patch_remove().to_debug_sql(),
        "{ op: 'remove', path: '/time/connected' }"
    );
    assert!(
        org.name
            .patch_change("@@ -1,4 +1,4 @@\n te\n-s\n+x\n t\n")
            .to_debug_sql()
            .starts_with("{ op: 'change', path: '/name', value: '")
    );
}

#[test]
fn patch_operations_reject_paths_with_clauses() {
    let filtered = Organization::schema()
        .users(User::schema().name.equal("Alice"))
        .name
        .patch_remove();
    assert!(filtered.to_query().is_err());

    let indexed = Organization::schema().users([0usize]).name.patch_remove();
    assert!(indexed.to_query().is_err());
}

/// A field of the same type is accepted as a value; one of another type does not compile
/// (`tests/ui/fail/assign_mismatched_field.rs`), and an explicit `Field` stays untyped.
#[test]
fn fields_stand_for_values_of_their_own_type() {
    let org = Organization::schema();
    let input = umbriel::statements::input_of::<Organization>();
    assert_eq!(
        org.age.increment_by(input.age).to_debug_sql(),
        "age += $input.age"
    );
    assert_eq!(
        org.name.equal_to(&User::schema().name).to_debug_sql(),
        "name = name"
    );
    assert_eq!(
        org.name.equal_to(Field::new("age")).to_debug_sql(),
        "name = age"
    );

    // A generic model's specialization is part of its field types: `Planet<u64>`'s `data`
    // fits `Planet<u64>`; `Planet<u32>`'s does not compile
    // (`tests/ui/fail/field_of_another_specialization.rs`).
    let planet = Planet::<u64>::schema();
    assert_eq!(
        planet
            .data
            .equal_to(Planet::<u64>::schema().data)
            .to_debug_sql(),
        "data = data"
    );
    assert_eq!(
        umbriel::statements::update::<Planet<u64>>(Planet::<u64>::id("earth"))
            .return_value(Planet::<u64>::schema().data)
            .to_debug_sql(),
        "UPDATE planet:earth RETURN VALUE data;"
    );
}

/// The SDK's `PatchOps` holds a list of its `PatchOp`s: Umbriel's operations convert into it
/// and back through the per-operation conversions, with no direct conversion of their own.
#[test]
fn patch_operation_lists_convert_to_and_from_the_sdks_patch_ops() {
    use surrealdb::opt::{PatchOp as SdkPatchOp, PatchOps};
    use umbriel::PatchOp;

    let org = Organization::schema();
    let ops = vec![
        org.name.patch_add("Acme Corp"),
        org.name.patch_replace("Globex"),
        org.name.patch_remove(),
    ];
    let sdk: PatchOps = ops
        .into_iter()
        .map(SdkPatchOp::try_from)
        .collect::<Result<Vec<_>, _>>()
        .expect("supported")
        .into();
    let back = sdk
        .into_iter()
        .map(PatchOp::from)
        .map(|op| op.to_debug_sql())
        .collect::<Vec<_>>();
    assert_eq!(
        back,
        [
            "{ op: 'add', path: '/name', value: 'Acme Corp' }",
            "{ op: 'replace', path: '/name', value: 'Globex' }",
            "{ op: 'remove', path: '/name' }",
        ]
    );
}

#[test]
fn patch_operations_convert_to_and_from_the_sdk() {
    use surrealdb::opt::PatchOp as SdkPatchOp;
    use umbriel::{PatchOp, PatchOpConversionError};

    let org = Organization::schema();
    let add = SdkPatchOp::try_from(org.name.patch_add("Acme Corp")).expect("add");
    assert!(matches!(&add, SdkPatchOp::Add { path, .. } if path == "/name"));
    assert!(matches!(
        SdkPatchOp::try_from(org.name.patch_remove()),
        Ok(SdkPatchOp::Remove { path }) if path == "/name"
    ));
    assert!(matches!(
        SdkPatchOp::try_from(org.name.patch_replace("Globex")),
        Ok(SdkPatchOp::Replace { .. })
    ));
    assert!(matches!(
        SdkPatchOp::try_from(org.name.patch_change("@@ -1 +1 @@\n-a\n+b\n")),
        Ok(SdkPatchOp::Change { .. })
    ));
    for unsupported in [
        org.name.patch_copy(Field::new("label")),
        org.name.patch_move(Field::new("label")),
        org.name.patch_test("Acme Corp"),
    ] {
        assert!(matches!(
            SdkPatchOp::try_from(unsupported),
            Err(PatchOpConversionError::Unsupported(_))
        ));
    }
    assert_eq!(
        SdkPatchOp::try_from(Organization::schema().users([0usize]).name.patch_remove()).err(),
        Some(PatchOpConversionError::InvalidPath)
    );

    assert_eq!(
        PatchOp::from(add).to_debug_sql(),
        "{ op: 'add', path: '/name', value: 'Acme Corp' }"
    );
    assert_eq!(
        PatchOp::from(SdkPatchOp::Remove {
            path: "/name".to_owned()
        })
        .to_debug_sql(),
        "{ op: 'remove', path: '/name' }"
    );
}
