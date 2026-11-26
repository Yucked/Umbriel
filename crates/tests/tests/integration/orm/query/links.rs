//! Links (`orm/src/query/links.rs`): single and many links, what is stored and decoded, and
//! loading linked records.

use umbriel_tests::{support::models::*, *};

#[test]
fn single_links_hold_an_id_a_fetched_record_or_nothing() {
    let sword = weapon(1, 10.0);

    let by_id = LinkOne::from(sword.id.clone());
    assert_eq!(by_id.key(), Some(&sword.id));
    assert!(by_id.value().is_none());

    let by_model = LinkOne::from(&sword);
    assert_eq!(by_model.key(), Some(&sword.id));

    let null = LinkOne::<Weapon>::default();
    assert!(null.key().is_none());
    assert!(matches!(*null, Reference::Null));
}

#[test]
fn single_links_store_the_id_and_decode_any_form() {
    let sword = weapon(1, 10.0);
    let fetched = LinkOne::<Weapon>::from_value(sword.clone().into_value()).expect("record");
    assert_eq!(fetched.value().map(|weapon| weapon.strength), Some(10.0));
    assert_eq!(fetched.key(), Some(&sword.id));
    // A fetched record is stored as its id.
    assert_eq!(
        fetched.into_value(),
        Value::RecordId(sword.id.clone().into_inner())
    );

    assert!(
        LinkOne::<Weapon>::from_value(Value::None)
            .expect("none")
            .key()
            .is_none()
    );
    assert!(
        LinkOne::<Weapon>::from_value(Value::Null)
            .expect("null")
            .key()
            .is_none()
    );
    assert_eq!(LinkOne::<Weapon>::null().into_value(), Value::None);
    assert!(
        LinkOne::<Weapon>::from_value(Value::RecordId(types::RecordId::new("alien", "x"))).is_err()
    );
}

#[test]
fn many_links_report_ids_and_fetched_records() {
    let ships = LinkMany::from(vec![SpaceShip::id("apollo"), SpaceShip::id("gemini")]);
    assert_eq!(ships.keys_truthy().len(), 2);
    assert_eq!(ships.keys().len(), 2);
    assert_eq!(ships.values_truthy_count(), 0);
    assert!(ships.keys_checked().is_empty());
    assert!(ships.values().iter().all(Option::is_none));

    let fetched = LinkMany::<SpaceShip>::from_value(Value::Array(
        vec![space_ship("apollo", "Apollo").into_value(), Value::None].into(),
    ))
    .expect("records");
    assert_eq!(fetched.values_truthy_count(), 1);
    assert_eq!(fetched.keys_checked(), [&SpaceShip::id("apollo")]);
    assert_eq!(fetched.keys(), [Some(&SpaceShip::id("apollo")), None]);

    assert!(
        LinkMany::<SpaceShip>::from_value(Value::None)
            .expect("none")
            .is_empty()
    );
}

#[test]
fn relations_are_never_written() {
    assert_eq!(Relate::<User>::null().into_value(), Value::None);
    assert!(
        Relate::<User>::from_value(Value::None)
            .expect("none")
            .is_empty()
    );
}

async fn armed_alien(db: &Surreal<Any>) {
    let sword = weapon(1, 10.0);
    sword.clone().create(db).await.expect("weapon");
    space_ship("apollo", "Apollo")
        .create(db)
        .await
        .expect("apollo");
    space_ship("gemini", "Gemini")
        .create(db)
        .await
        .expect("gemini");
    alien("bob", "Bob").create(db).await.expect("bob");

    Alien {
        ally: LinkSelf::from(Alien::id("bob")),
        weapon: LinkOne::from(sword.id),
        space_ships: LinkMany::from(vec![SpaceShip::id("apollo"), SpaceShip::id("gemini")]),
        ..alien("alice", "Alice")
    }
    .create(db)
    .await
    .expect("alice");
}

fn alice() -> umbriel::statements::SelectStatement<Alien> {
    Alien::find_where(Alien::schema().name.equal("Alice"))
}

/// The one record `statement` reads.
async fn one(statement: umbriel::statements::SelectStatement<Alien>, db: &Surreal<Any>) -> Alien {
    let mut found = statement.execute(db).await.expect("read");
    assert_eq!(found.len(), 1, "one record");
    found.remove(0)
}

#[tokio::test]
async fn links_are_ids_until_loaded() {
    let (db, _guard) = db().await;
    armed_alien(&db).await;

    let alice = one(alice(), &db).await;
    assert_eq!(alice.weapon.key(), Some(&Weapon::id(1)));
    assert!(alice.weapon.value().is_none());
    assert_eq!(alice.ally.key(), Some(&Alien::id("bob")));
    assert_eq!(alice.space_ships.keys_truthy().len(), 2);
    assert_eq!(alice.space_ships.values_truthy_count(), 0);
}

#[tokio::test]
async fn load_all_links_fetches_every_kind_of_link() {
    let (db, _guard) = db().await;
    armed_alien(&db).await;

    let alice = one(alice().load_all_links(), &db).await;
    assert_eq!(alice.weapon.value().map(|w| w.strength), Some(10.0));
    assert_eq!(
        alice.ally.value().map(|ally| ally.name.as_str()),
        Some("Bob")
    );
    let names = alice
        .space_ships
        .values_truthy()
        .into_iter()
        .map(|ship| ship.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Apollo", "Gemini"]);
}

#[tokio::test]
async fn load_by_link_kind() {
    let (db, _guard) = db().await;
    armed_alien(&db).await;

    let ones = one(
        alice()
            .load_links(Alien::get_link_one_fields())
            .expect("links"),
        &db,
    )
    .await;
    assert!(ones.weapon.value().is_some());
    assert!(ones.ally.value().is_none());
    assert_eq!(ones.space_ships.values_truthy_count(), 0);

    let selfs = one(
        alice()
            .load_links(Alien::get_link_self_fields())
            .expect("links"),
        &db,
    )
    .await;
    assert!(selfs.ally.value().is_some());
    assert!(selfs.weapon.value().is_none());

    let manys = one(
        alice()
            .load_links(Alien::get_link_many_fields())
            .expect("links"),
        &db,
    )
    .await;
    assert_eq!(manys.space_ships.values_truthy_count(), 2);
    assert!(manys.weapon.value().is_none());
}

#[tokio::test]
async fn loading_a_link_to_a_missing_record_gives_nothing() {
    let (db, _guard) = db().await;
    Alien {
        weapon: LinkOne::from(Weapon::id(404)),
        ..alien("alice", "Alice")
    }
    .create(&db)
    .await
    .expect("alice");

    let alice = one(
        alice()
            .load_links(Alien::get_link_one_fields())
            .expect("links"),
        &db,
    )
    .await;
    assert!(alice.weapon.value().is_none());
}

#[test]
fn load_links_rejects_fields_that_are_not_links() {
    let result = alice().load_links(vec![Field::new("name")]);
    assert!(matches!(
        result,
        Err(umbriel::Error::FieldsUnfetchableNotARecordLink(_))
    ));
    assert!(alice().load_links(vec![Alien::schema().weapon]).is_ok());
}

/// Typed link paths, from `Links::links()`, render the fields they walk.
mod link_paths {
    use umbriel::{Links, statements::SelectStatement};

    use super::*;

    #[test]
    fn link_paths_render_each_hop() {
        assert_eq!(Member::links().wallet.to_debug_sql(), "wallet");
        assert_eq!(
            Member::links()
                .wallet
                .then(Wallet::links().bank)
                .to_debug_sql(),
            "wallet.bank"
        );
        assert_eq!(
            Club::links()
                .members
                .then(Member::links().wallet)
                .to_debug_sql(),
            "members.wallet"
        );
        assert_eq!(
            Member::links()
                .mentor
                .then(Member::links().wallet)
                .to_debug_sql(),
            "mentor.wallet"
        );
        assert_eq!(
            Club::links()
                .members
                .then(Member::links().wallet)
                .then(Wallet::links().bank)
                .to_debug_sql(),
            "members.wallet.bank"
        );
    }

    #[test]
    fn a_renamed_link_renders_its_database_name() {
        assert_eq!(Member::links().employer.to_debug_sql(), "worksFor");
        assert_eq!(Alien::links().space_ships.to_debug_sql(), "spaceShips");
    }

    #[test]
    fn chained_fetch_links_render_one_clause() {
        let statement: SelectStatement<Member> = Member::select()
            .fetch_link(Member::links().wallet.then(Wallet::links().bank))
            .fetch_link(Member::links().employer);
        assert_sql(
            &statement,
            "SELECT * FROM member FETCH wallet.bank, worksFor;",
        );
    }

    #[test]
    fn a_repeated_path_renders_each_time() {
        assert_sql(
            &Member::select()
                .fetch_link(Member::links().wallet)
                .fetch_link(Member::links().wallet),
            "SELECT * FROM member FETCH wallet, wallet;",
        );
    }
}
