//! Clauses (`orm/src/query/clause.rs`): link and graph steps.

use umbriel_tests::{support::query::*, *};


    #[test]
    fn link_steps() {
        let student = Student::schema();
        assert_eq!(
            student.semesterCourses(E).title.to_debug_sql(),
            "semesterCourses.title"
        );
        assert_eq!(
            student.semesterCourses(All).title.to_debug_sql(),
            "semesterCourses[*].title"
        );
        assert_eq!(
            student.semesterCourses(Last).title.to_debug_sql(),
            "semesterCourses[$].title"
        );
        assert_eq!(
            student.semesterCourses(index(2)).title.to_debug_sql(),
            "semesterCourses[2].title"
        );
    }

    #[test]
    fn graph_steps() {
        let likes = Table::new("likes");
        let reads = Table::new("reads");
        let path = Student::with(Student::id("alice"))
            .writes__(any_other_edges(&[likes, reads]).where_(Field::new("count").lte(50)))
            .book(E);
        let path = Field::from(path).to_debug_sql();
        assert_eq!(
            path,
            "student:alice->(writes, likes, reads WHERE count <= 50)->book"
        );
        assert_parses(&format!("SELECT {path} FROM student;"));
        let to_book = Student::with(Student::id("alice"))
            .writes__(E)
            .book(Book::id("rust"));
        assert_eq!(
            Field::from(to_book).to_debug_sql(),
            "student:alice->writes->book:rust"
        );
        // A typed id of another model does not compile; an untyped one is checked on rendering.
        let wrong = Student::with(Student::id("alice"))
            .writes__(E)
            .book(types::RecordId::from(Blog::id("rust")));
        assert!(Field::from(wrong).to_query().is_err());
        assert_eq!(EdgeDirection::Out.as_arrow_symbol().to_debug_sql(), "->");
        assert_eq!(EdgeDirection::In.as_arrow_symbol().to_debug_sql(), "<-");
    }
