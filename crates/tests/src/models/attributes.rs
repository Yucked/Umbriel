//! Models exercising every `#[orm(…)]` definition setting, with values given inline, as paths to
//! functions and as function calls.

use std::time::Duration;

use umbriel::{
    CrudType::{self, *},
    Field, Filter, LinkMany, LinkOne, LinkSelf, Model, NONE, Node, Operatable, RecordId, Relate,
    SchemaGetter, Table, cond, model,
    statements::{
        DefineFieldStatement, DefineTableStatement, Permissions, SelectStatement, define_field,
        define_table, for_permission, order, select,
    },
    types::Kind,
    value,
};

pub fn age_permissions() -> Permissions {
    let schema = StudentWithGranularAttributes::schema();
    [
        for_permission([Create, Delete]).where_(schema.firstName.is("Alice")),
        for_permission(Update).where_(schema.ageInlineExpr.less_than_or_equal(130)),
    ]
    .into()
}

pub fn student_permissions() -> Permissions {
    let schema = StudentWithGranularAttributes::schema();
    vec![
        for_permission([Select, Update]).where_(schema.firstName.is("Bob")),
        for_permission([Create, Delete]).where_(schema.ageInlineExpr.lte(57)),
    ]
    .into()
}

pub fn default_duration_value() -> Duration {
    Duration::from_secs(60 * 60 * 24 * 7)
}

pub fn get_age_default_value() -> u8 {
    18
}

pub fn get_age_assertion() -> Filter {
    cond(value().is_not(NONE)).and(value().gte(18))
}

pub enum AgeGroup {
    Child,
    Teen,
    Adult,
    Senior,
}

pub fn get_age_by_group_default_value(group: AgeGroup) -> u8 {
    match group {
        AgeGroup::Child => 10,
        AgeGroup::Teen => 18,
        AgeGroup::Adult => 30,
        AgeGroup::Senior => 60,
    }
}

pub fn as_fn() -> SelectStatement {
    select(umbriel::All).from(Student::table())
}

/// Table settings given as paths to functions. `as_` makes it a view.
#[model(view, table = "student_fn_attrs")]
#[orm(drop, schemafull, as_ = as_fn, permissions = student_permissions)]
#[derive(Debug, Clone)]
pub struct StudentFnAttrs {
    pub id: RecordId<Self>,
}

/// Table and field settings given inline, as paths and as calls.
#[model(node, table = "student_with_granular_attributes")]
#[orm(
    drop,
    schemafull,
    permissions = student_permissions()
)]
#[surreal(rename_all = "camelCase")]
#[derive(Debug, Clone)]
pub struct StudentWithGranularAttributes {
    pub id: RecordId<Self>,
    pub first_name: String,
    pub last_name: String,
    #[orm(
        ty = "int",
        value = 18,
        assert = cond(value().is_not(NONE)).and(value().gte(18)),
        permissions = for_permission([CrudType::Create, CrudType::Delete]).where_(StudentWithGranularAttributes::schema().firstName.is("Alice"))
    )]
    pub age_inline_expr: u8,

    #[orm(
        ty = "int",
        value = get_age_default_value(),
        assert = get_age_assertion,
        permissions = age_permissions
    )]
    pub age_default_external_function_invoked_expr: u8,

    #[orm(
        ty = "int",
        value = get_age_by_group_default_value(AgeGroup::Teen),
        assert = get_age_assertion,
        permissions = age_permissions()
    )]
    pub age_teen_external_function_invoked_expr: u8,

    #[orm(
        ty = "int",
        value = get_age_by_group_default_value(AgeGroup::Senior),
        assert = get_age_assertion
    )]
    pub age_senior_external_function_invoked_expr: u8,

    #[orm(
        ty = "int",
        value = get_age_by_group_default_value(AgeGroup::Child),
        permissions = age_permissions
    )]
    pub age_child_external_function_invoked_expr: u8,

    #[orm(
        ty = "int",
        value = get_age_by_group_default_value(AgeGroup::Adult)
    )]
    pub age_adult_external_function_invoked_expr: u8,

    #[orm(
        ty = "int",
        value = get_age_default_value,
        assert = get_age_assertion,
        permissions = age_permissions
    )]
    pub age_external_fn_attrs: u8,

    #[orm(
        ty = "duration",
        value = default_duration_value,
        assert = value().is_not(NONE)
    )]
    pub time_to_kelowna: Duration,

    #[orm(
        ty = "duration",
        value = Duration::from_secs(60 * 60 * 24 * 7),
        assert = value().is_not(NONE)
    )]
    pub time_to_kelowna_inline: Duration,

    #[orm(
        link_self = "StudentWithGranularAttributes",
        ty = "record<student_with_granular_attributes>"
    )]
    pub best_friend: LinkSelf<StudentWithGranularAttributes>,

    #[orm(link_one = "Book")]
    #[surreal(rename = "unoBook")]
    pub fav_book: LinkOne<Book>,

    #[orm(link_one = "Book")]
    pub course: LinkOne<Book>,

    #[orm(link_many = "Book", ty = "array<record<book>>", item_assert = value().is_not(NONE))]
    #[surreal(rename = "semesterCourses")]
    pub all_semester_courses: LinkMany<Book>,

    #[orm(relate(
        model = "StudentWithGranularAttributesWritesBook",
        connection = "->writes->book"
    ))]
    pub written_books: Relate<Book>,
}

pub type StudentWithGranularAttributesWritesBook = Writes<StudentWithGranularAttributes, Book>;

pub fn define_first_name(field: impl Into<Field>, table: Table) -> DefineFieldStatement {
    let schema = StudentWithDefineAttr::schema();
    define_field(field)
        .on_table(table)
        .type_(Kind::String)
        .default("Alice")
        .assert(cond(value().is_not(NONE)).and(value().contains("A")))
        .permissions([
            for_permission(Select).where_(schema.ageDefineInline.gte(18)),
            for_permission([Create, Update]).where_(schema.firstName.is("Bob")),
        ])
}

pub fn define_last_name() -> DefineFieldStatement {
    let schema = StudentWithDefineAttr::schema();
    define_field(&schema.lastName)
        .on_table(StudentWithDefineAttr::table())
        .type_(Kind::String)
        .default("Bob")
        .assert(cond(value().is_not(NONE)).and(value().contains("B")))
        .permissions([
            for_permission(Select).where_(schema.ageDefineInline.gte(18)),
            for_permission([Create, Update]).where_(schema.lastName.is("Bob")),
        ])
}

pub fn define_age(field: impl Into<Field>) -> DefineFieldStatement {
    let schema = Student::schema();
    define_field(field)
        .on_table(Student::table())
        .type_(Kind::Int)
        .default(18)
        .assert(cond(value().is_not(NONE)).and(value().gte(18)))
        .permissions(for_permission(Select).where_(schema.age.greater_than_or_equal(18)))
        .permissions(for_permission([Create, Update]).where_(schema.firstName.is("Bob")))
        .permissions([
            for_permission([Create, Delete]).where_(schema.firstName.is("Alice")),
            for_permission(Update).where_(schema.age.less_than_or_equal(130)),
        ])
}

pub fn define_student_with_define_attr() -> DefineTableStatement {
    let schema = Student::schema();
    define_table(StudentWithDefineAttr::table())
        .drop()
        .as_(
            select(umbriel::All)
                .from(Student::table())
                .where_(schema.firstName.is("Alice"))
                .order_by(order(&schema.age).numeric().desc())
                .limit(20)
                .start(5),
        )
        .schemafull()
        .permissions(for_permission(Select).where_(schema.age.greater_than_or_equal(18)))
        .permissions(for_permission([Create, Delete]).where_(schema.lastName.is("Bob")))
        .permissions([
            for_permission([Create, Delete]).where_(schema.lastName.is("Bob")),
            for_permission(Update).where_(schema.age.less_than_or_equal(130)),
        ])
}

/// Whole definitions given by `define`, as calls and as paths.
#[model(node, table = "student_with_define_attr")]
#[orm(define = define_student_with_define_attr())]
#[surreal(rename_all = "camelCase")]
#[derive(Debug, Clone)]
pub struct StudentWithDefineAttr {
    pub id: RecordId<Self>,
    #[orm(
        ty = "string",
        define = define_first_name(StudentWithDefineAttr::schema().firstName, StudentWithDefineAttr::table())
    )]
    pub first_name: String,

    #[orm(ty = "string", define = define_last_name)]
    pub last_name: String,

    #[orm(
        ty = "int",
        define = define_field(StudentWithDefineAttr::schema().ageDefineInline).on_table(Student::table()).type_(Kind::Int).default(18)
    )]
    pub age_define_inline: u8,

    #[orm(
        ty = "int",
        define = define_age(StudentWithDefineAttr::schema().ageDefineExternalInvoke)
    )]
    pub age_define_external_invoke: u8,
}

/// The table definition given as a path.
#[model(node, table = "student_with_define_fn_attr")]
#[orm(define = define_student_with_define_attr)]
#[derive(Debug, Clone)]
pub struct StudentWithDefineFnAttr {
    pub id: RecordId<Self>,
}

#[model(node, table = "student")]
#[surreal(rename_all = "camelCase")]
#[derive(Debug, Clone, Default)]
pub struct Student {
    pub id: RecordId<Self>,
    pub first_name: String,
    pub last_name: String,
    #[orm(
        ty = "int",
        value = 18,
        assert = cond(value().is_not(NONE)).and(value().gte(18)),
        permissions = age_permissions
    )]
    pub age: u8,

    #[orm(ty = "int")]
    pub score: u8,

    #[orm(link_self = "Student", ty = "record<student>")]
    pub best_friend: LinkSelf<Student>,

    #[orm(link_one = "Book")]
    #[surreal(rename = "unoBook")]
    pub fav_book: LinkOne<Book>,

    #[orm(link_one = "Book")]
    pub course: LinkOne<Book>,

    #[orm(link_many = "Book", ty = "array<record<book>>")]
    #[surreal(rename = "semesterCourses")]
    pub all_semester_courses: LinkMany<Book>,

    #[orm(relate(model = "StudentWritesBook", connection = "->writes->book"))]
    pub written_books: Relate<Book>,

    #[orm(relate(model = "StudentWritesBlog", connection = "->writes->blog"))]
    pub blogs: Relate<Blog>,
}

#[model(edge, table = "writes")]
#[surreal(rename_all = "camelCase")]
#[derive(Debug, Clone, Default)]
pub struct Writes<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,

    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,

    pub time_written: Duration,
    pub count: i32,
}

pub type StudentWritesBook = Writes<Student, Book>;
pub type StudentWritesBlog = Writes<Student, Blog>;

#[model(edge, table = "likes")]
#[surreal(rename_all = "camelCase")]
#[derive(Debug, Clone, Default)]
pub struct Likes<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[surreal(rename = "in")]
    #[orm(link_one = "In")]
    pub in_: LinkOne<In>,

    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,

    pub likes_count: u64,
}

pub type StudentLikesBook = Likes<Student, Book>;

#[model(node, table = "book")]
#[derive(Debug, Clone, Default)]
pub struct Book {
    pub id: RecordId<Self>,
    pub title: String,
    pub content: String,
}

#[model(node, table = "blog")]
#[derive(Debug, Clone, Default)]
pub struct Blog {
    pub id: RecordId<Self>,
    pub title: String,
    pub content: String,
}
