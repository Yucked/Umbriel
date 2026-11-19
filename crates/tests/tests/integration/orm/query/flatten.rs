//! Flattened objects (`orm/src/query/flatten.rs`): an object stored as fields of the model that
//! holds it.

use umbriel_tests::{support::models::*, *};


    fn customer(street: &str) -> Customer {
        Customer {
            id: Customer::id("alice"),
            name: "Alice".to_owned(),
            address: Address {
                street: street.to_owned(),
                city: "Springfield".to_owned(),
            },
        }
    }

    #[test]
    fn flattened_fields_are_the_models_own() {
        assert_eq!(Customer::schema().address.street.to_debug_sql(), "street");
        assert_eq!(
            joined(&Customer::define_fields()),
            "DEFINE FIELD name ON TABLE customer TYPE string;
DEFINE FIELD street ON TABLE customer TYPE string;
DEFINE FIELD city ON TABLE customer TYPE string;"
        );
    }

    #[test]
    fn a_flat_patch_assigns_every_field_of_the_object() {
        let patch = CustomerPatch {
            address: FlatPatch::Set(Address {
                street: "2 Side St".to_owned(),
                city: "Shelbyville".to_owned(),
            }),
            ..Default::default()
        };
        assert_eq!(
            patch.into_setters().to_debug_sql(),
            "street = '2 Side St', city = 'Shelbyville'"
        );
        assert!(CustomerPatch::default().into_setters().is_empty());
    }

    #[tokio::test]
    async fn flattened_objects_round_trip() {
        let (db, _guard) = db().await;
        customer("1 Main St").save(&db).await.expect("saved");

        let street: String = read(&db, "RETURN customer:alice.street;").await;
        assert_eq!(street, "1 Main St");
        let found = Customer::get(&db, &Customer::id("alice"))
            .await
            .expect("read")
            .expect("exists");
        assert_eq!(found.address, customer("1 Main St").address);

        let patched = Customer::patch(
            &db,
            &Customer::id("alice"),
            CustomerPatch {
                address: FlatPatch::Set(Address {
                    street: "2 Side St".to_owned(),
                    city: "Springfield".to_owned(),
                }),
                ..Default::default()
            },
        )
        .await
        .expect("patched")
        .expect("exists");
        assert_eq!(patched.address.street, "2 Side St");
    }
