use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr, parse_macro_input};

mod embed_migrations;
mod helpers;
mod kind;
mod migrations;
mod models;
mod models_union;
mod query;
mod query_builder;
mod statement_parser;
mod surql;
mod utilities;

use migrations::table::TableMigrationSchemaDeriveAttributes;
use models::ModelReceiver;
use utilities::{PickedMeta, TableDeriveAttributesPickable};

/// Expands a darling-parsed derive input, reporting attribute errors at their spans.
fn derive_with<T: FromDeriveInput + quote::ToTokens>(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match T::from_derive_input(&input) {
        Ok(output) => quote!(#output).into(),
        Err(err) => err.write_errors().into(),
    }
}

/// Declares a model; documented on its re-export, `umbriel::model`.
#[proc_macro_attribute]
pub fn model(args: TokenStream, item: TokenStream) -> TokenStream {
    models::model_attribute(args.into(), item.into()).into()
}

/// Generates a model's code; added by `#[umbriel::model]`, which passes the model kind and
/// table as `#[orm(..)]`. `surreal` is declared too so `value = manual` models keep their
/// `#[surreal(rename …)]` attributes.
#[doc(hidden)]
#[proc_macro_derive(ModelCodegen, attributes(orm, surreal))]
pub fn model_codegen(input: TokenStream) -> TokenStream {
    derive_with::<ModelReceiver>(input)
}

#[proc_macro_derive(TableResources, attributes(orm))]
pub fn table_resources_derive(input: TokenStream) -> TokenStream {
    derive_with::<TableMigrationSchemaDeriveAttributes>(input)
}

/// Declares a union of models for mixed-model writes; documented on its re-export,
/// `umbriel::Models`.
#[proc_macro_derive(Models, attributes(models))]
pub fn models_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    models_union::derive_models(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Pickable)]
pub fn pickable_derive(input: TokenStream) -> TokenStream {
    derive_with::<TableDeriveAttributesPickable>(input)
}

#[proc_macro]
pub fn pick(input: TokenStream) -> TokenStream {
    match syn::parse::<PickedMeta>(input) {
        Ok(output) => quote!(#output).into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn embed_migrations(input: TokenStream) -> TokenStream {
    embed_migrations::generate_embedded_migrations(input.into()).into()
}

/// Checks a query at compile time and returns the query as a string.
/// Unlike, query! macro, this macro does not allow variable interpolation.
///
/// # Validation
/// Syntax only, using SurrealDB's own parser at the version Umbriel depends on: tables, fields,
/// permissions and parameter types are not checked, and syntax newer than that version is rejected.
///
/// # Arguments
/// * `query` - The query to be checked at compile time.
///
/// # Example
/// ```rust, ignore
/// use umbriel::{query_raw, statements::select};
///
/// let query = query_raw!("SELECT name, age, * FROM users");
/// let query = query_raw!("SELECT name, age, * FROM users WHERE name = $1 AND name = 'Charlie'");
/// let query = query_raw!("SELECT ->purchased->product<-purchased<-person->purchased->product FROM person:alice");
/// ```
#[proc_macro]
pub fn query_raw(raw_input: TokenStream) -> TokenStream {
    let r_input = raw_input.clone();
    let literal = parse_macro_input!(r_input as LitStr);
    match surql::validate(&literal.value(), literal.span()) {
        Ok(()) => raw_input,
        Err(err) => err.to_compile_error().into(),
    }
}

/// Checks one or multiple quer(ies) at compile time.
/// It also allows interpolations of variables and can
/// be run.
///
/// # Validation
/// Syntax only, using SurrealDB's own parser at the version Umbriel depends on: tables, fields,
/// permissions and parameter types are not checked, and syntax newer than that version is rejected.
/// Every `$placeholder` must have a binding and every binding a placeholder.
///
/// # Arguments
/// * `db` - The database connection to be used.
/// * `quer(ies)` - A single or list of queries to be checked at compile time.
/// * `params` - The parameters to be used for the query.
///
/// # Example
/// ```rust, ignore
/// use umbriel::{query};
/// use surrealdb::{engine::local::Mem, Surreal};
///
/// let db = Surreal::new::<Mem>(()).await.unwrap();
/// db.use_ns("test").use_db("test").await.unwrap();
///
/// let _query = query!(db, "SELECT * FROM users").await;
/// let _query = query!(db, "SELECT * FROM users", {}).await;
/// let _query = query!(db, "SELECT * FROM users WHERE id = $id", {id : 1} ).await;
/// let username = "Charlie";
/// let _query = query!(db, "SELECT name, age FROM users WHERE id = $id AND name = $name", {
///     id : 1,
///     name : username
/// })
/// .await;
///
/// ```
///
/// Also supports multiple queries in a single call.
/// ```rust, ignore
/// use umbriel::{query};
/// use surrealdb::{engine::local::Mem, Surreal};
///
/// let db = Surreal::new::<Mem>(()).await.unwrap();
/// db.use_ns("test").use_db("test").await.unwrap();
///
/// let _queries = query!(
///     db,
///     [
///         "SELECT * FROM users WHERE id = $id",
///         "CREATE user:charlie SET name = $name, company = 'ExampleCo', skills = $skills"
///     ],
///     {
///         id: 1,
///         name: "Charlie",
///         skills: vec!["Rust", "python", "typescript"]
///     }
/// )
/// .await;
/// ```
#[proc_macro]
pub fn query(raw_input: TokenStream) -> TokenStream {
    query::query(raw_input.into()).into()
}

/// A novel approach to writing SQL queries in Rust. It allows you to write SQL queries
/// in a native-like Rust Syntax. It supports mixing query builder functions with native looking
/// syntax such as if, for, let, return statements and standard Rust expressions which can be
/// a normal umbriel query builder function statement.
///
/// It dynamically returns a query chain, transaction or a block depending on the usage.
/// By default, it returns a query chain. If you want to return a transaction, you start
/// with a `begin transaction;` and end with a `commit transaction;` or `cancel transaction;`
/// keywords.
/// To return a block, you have to return typically at the end.
///
/// # Arguments
/// * `queries` - Series of queries to be checked at compile time.
///
/// # Example
///
/// ```rust, ignore
/// chain! {
///     let balance1 = create_only().content(Balance {
///     id: Balance::create_id("balance1".to_string()),
///         amount: amount_to_transfer,
///     });
///
///     create_only().content(Balance {
///         id: Balance::create_id("balance2".to_string()),
///         amount: amount_to_transfer,
///     });
///
///     if balance.greater_than(100) {
///         let first_name = "Charlie";
///         let score = 100;
///         select(All).from(Account::table()).where_(acc.balance.eq(5));
///     } else if balance.gte(100) {
///         let first_name = "Charlie";
///         let score = 100;
///         select(All).from(Account::table()).where_(acc.balance.eq(5));
///     } else {
///         let first_name = "Charlie";
///         let score = 100;
///         select(All).from(Account::table()).where_(acc.balance.eq(5));
///     };
///
///     for name in vec!["Charlie", "Bennett"] {
///         let first = "Charlie";
///         select(All).from(Account::table()).where_(acc.balance.eq(5));
///         
///         let good_stmt = select(All).from(Account::table()).where_(acc.balance.eq(64));
///
///         if balance.gt(50) {
///             let first_name = "Charlie";
///         };
///
///         select(All).from(Account::table()).where_(acc.balance.eq(34));
///
///         let numbers = vec![23, 98];
///
///         for age in numbers {
///             let score = 100;
///             let first_stmt = select(All).from(Account::table()).where_(acc.balance.eq(5));
///
///             let second_stmt = select(All).from(Account::table()).where_(acc.balance.eq(25));
///             select(All).from(Account::table()).where_(acc.balance.eq(923));
///             
///         };
///     };
///
///     // You can reference the balance object by using the $balance variable and pass the amount
///     // as a parameter to the decrement_by function. i.e $balance.amount
///     let updated1 = update::<Account>(id1).set(acc.balance.increment_by(balance1.with_path::<Balance>(E).amount));
///     update::<Account>(id1).set(acc.balance.increment_by(balance1.with_path::<Balance>(E).amount));
///     update::<Account>(id1).set(acc.balance.increment_by(45.3));
///     
///     // You can also pass the amount directly to the decrement_by function. i.e 300.00
///     update::<Account>(id2).set(acc.balance.decrement_by(amount_to_transfer));
///     update::<Account>(id2).set(acc.balance.decrement_by(50));
/// };
///
/// // as transaction.
/// chain! {
///    BEGIN TRANSACTION;
///    let balance1 = create_only().content(Balance {
///         id: Balance::create_id("balance1".to_string()),
///         amount: amount_to_transfer,
///    });
///
///    COMMIT TRANSACTION;
/// };
///
/// // as block.
/// chain! {
///   let result = select(All).from(Account::table()).where_(acc.balance.eq(5));
///
///   return result;
/// };
/// ```
#[proc_macro]
pub fn chain(input: TokenStream) -> TokenStream {
    query_builder::query_chain(input)
}

/// A block is a series of queries that are executed as a single
/// unit, surrounded by curly braces and typically returns a value. It can be assigned to a field or variable.
/// It supports mixing query builder functions with native looking
/// syntax such as if, for, let, return statements and standard Rust expressions which can be
/// a normal umbriel query builder function statement.
///
/// # Arguments
/// * `queries` - Series of queries to be checked at compile time.
///
/// # Example
/// ```rust, ignore
/// let created_stats_statement = create::<WeaponStats>().set(object_partial!(WeaponStats {
///     averageStrength: block! {
///         let strengths = select_value(strength).from(weapon);
///         let total = math::sum!(strengths); // 105
///         let count = array::len!(strengths);     // 15
///         return math::ceil!((((total / count) * (count * total)) / (total + 4)) * 100);
///     }
/// }));
/// ```
#[proc_macro]
pub fn block(input: TokenStream) -> TokenStream {
    query_builder::query_block(input)
}

/// A transaction is a series of queries that are executed as a single
/// unit. It supports mixing query builder functions with native looking
/// syntax such as if, for, let, return statements and standard Rust expressions which can be
/// a normal umbriel query builder function statement.
///
/// It must start with a `begin transaction;` and end with a `commit transaction;` or `cancel transaction;`
/// keywords.
///
/// # Arguments
/// * `queries` - Series of queries to be checked at compile time.
///
/// # Example
///
/// ```rust, ignore
///
/// let db = Surreal::new::<Mem>(()).await.unwrap();
/// db.use_ns("test").use_db("test").await.unwrap();
///
/// let id1 = &Account::create_id("one".to_string());
/// let id2 = &Account::create_id("two".to_string());
/// let amount_to_transfer = 300.00;
///
/// let acc = Account::schema();
/// transaction! {
///     BEGIN TRANSACTION;
///     
///     let balance = create_only().content(Balance {
///         id: Balance::create_id("balance1".into()),
///         amount: amount_to_transfer,
///     });
///
///     create_only().content(Account {
///         id: id1.clone(),
///         balance: 135_605.16,
///     });
///     create_only().content(Account {
///         id: id2.clone(),
///         balance: 91_031.31,
///     });
///
///     // You can reference the balance object by using the $balance variable and pass the amount
///     // as a parameter to the decrement_by function. i.e $balance.amount
///     update::<Account>(id1).set(acc.balance.increment_by(balance.with_path::<Balance>(E).amount));
///
///     // You can also pass the amount directly to the decrement_by function. i.e 300.00
///     update::<Account>(id2).set(acc.balance.decrement_by(amount_to_transfer));
///
///     COMMIT TRANSACTION;
/// }
/// .run(db.clone())
/// .await?;
/// ```
#[proc_macro]
pub fn transaction(input: TokenStream) -> TokenStream {
    query_builder::query_transaction(input)
}

/// A macro to generate a function definition statement and the corresponding helper function.
/// e.g. `define_function!(get_it(first: bool, last: string, birthday: string) { let person = "43"; return person; });`
/// generates a `get_it_statement` itself and `get_it` helper function created by the macro.
///
/// # Arguments
/// * `function definition` - The function definition
///
/// # Example
/// ```rust, ignore
/// use umbriel::{*, statements::define_function};
///
/// // The below generates a `get_it_statement` itself and `get_it` helper function created by the macro.
/// define_function!(get_it(first: bool, last: string, birthday: string) {
///     let person = "43";
///     return person;
/// });
/// ```
///
/// ```rust, ignore   
/// // The below generates a `get_or_create_spaceship_statement` itself and `get_or_create_spaceship` helper function created by the macro.
/// define_function!(get_or_create_spaceship(
///     first_arg: string,
///     last_arg: string,
///     birthday_arg: datetime,
///     _very_complex_type: int | option<float> | array<option<string>|int|null, 10> | set<option<number>|float|null, 10> | option<array> | option<set<option<int>>>
/// ) {
///     let person = select(All)
///         .from(SpaceShip::table())
///         .where_(
///             cond(SpaceShip::schema().id.equal(first_arg))
///                 .and(SpaceShip::schema().name.equal(last_arg))
///                 .and(SpaceShip::schema().created.equal(birthday_arg)),
///         );
///
///     if person.with_path::<SpaceShip>([0]).id.is_not(NONE) {
///         return person;
///     } else {
///         return create::<SpaceShip>().set(
///                     object!(SpaceShip {
///                         id: first_arg,
///                         name: last_arg,
///                         created: birthday_arg,
///                     })
///                 );
///     };
/// });
/// ```
#[proc_macro]
pub fn define_function(input: TokenStream) -> TokenStream {
    statement_parser::define_function(input)
}

/// A helper function to create a standalone for loop if you don't want to define within
/// chain! or block! or transaction! macro. This is almost never necessary. Just use
/// chain! or block! or transaction! macro and define your for loop within it,
///
/// The loop parameter is declared by the macro; refer to it by reference inside the body.
///
/// ```rust, ignore
/// use umbriel::{*, statements::select};
///
/// let person_table = &Table::from("person");
/// let user_name = &Field::new("user_name");
///
/// let for_loop = for_!((name in vec!["Alice", "Bob"]) {
///     select(All).from(person_table).where_(user_name.equal(&name));
/// };);
/// assert_eq!(
///     for_loop.to_debug_sql(),
///     "FOR $name IN ['Alice', 'Bob'] {\nSELECT * FROM person WHERE user_name = $name;\n};"
/// );
/// ```
#[proc_macro]
pub fn for_(input: TokenStream) -> TokenStream {
    statement_parser::for_::for_loop_without_for_keyword(input)
}
