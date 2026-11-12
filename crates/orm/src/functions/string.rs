//! String functions, `string::*`.
//!
//! ```rust
//! use umbriel::{*, functions::string};
//! let name = Field::new("name");
//! assert_eq!(string::len!(name.clone()).to_debug_sql(), "string::len(name)");
//! assert_eq!(StrandLike::from(name).uppercase().to_debug_sql(), "name.uppercase()");
//! ```

use crate::kinds;

functions! { $ "string" "::", string, string;
    /// Capitalises the first letter of every word.
    capitalize(string: StrandLike) -> kinds::String [StrandLike.capitalize];
    /// Concatenates the values as strings.
    concat(.. values: ValueLike) -> kinds::String [StrandLike.concat];
    /// Whether `string` contains `other`.
    contains(string: StrandLike, other: StrandLike) -> kinds::Bool [StrandLike.contains];
    /// Whether `string` ends with `suffix`.
    ends_with(string: StrandLike, suffix: StrandLike) -> kinds::Bool [StrandLike.ends_with];
    /// Joins the values with the first one as the separator.
    join(.. values: ValueLike) -> kinds::String [StrandLike.join];
    /// The number of characters.
    len(string: StrandLike) -> kinds::Int [StrandLike.len];
    /// The string in lower case.
    lowercase(string: StrandLike) -> kinds::String [StrandLike.lowercase];
    /// Whether `string` matches the regular expression `pattern`.
    matches(string: StrandLike, pattern: ValueLike) -> kinds::Bool [StrandLike.matches];
    /// The string repeated `count` times.
    repeat(string: StrandLike, count: NumberLike) -> kinds::String [StrandLike.repeat];
    /// Replaces each occurrence of `search` (a string or a regex) with `replacement`.
    replace(string: StrandLike, search: ValueLike, replacement: StrandLike) -> kinds::String
        [StrandLike.replace];
    /// The string reversed.
    reverse(string: StrandLike) -> kinds::String [StrandLike.reverse];
    /// The characters from the index `start` up to the index `end`, exclusive, or in the range
    /// `start`.
    slice(string: StrandLike; start: ValueLike, end: NumberLike) -> kinds::String
        [StrandLike.slice];
    /// A URL-friendly version of the string.
    slug(string: StrandLike) -> kinds::String [StrandLike.slug];
    /// The parts of `string` between occurrences of `delimiter`.
    split(string: StrandLike, delimiter: StrandLike) -> kinds::Array<kinds::String>
        [StrandLike.split];
    /// Whether `string` starts with `prefix`.
    starts_with(string: StrandLike, prefix: StrandLike) -> kinds::Bool
        [StrandLike.starts_with];
    /// The string without leading and trailing whitespace.
    trim(string: StrandLike) -> kinds::String [StrandLike.trim];
    /// The string in upper case.
    uppercase(string: StrandLike) -> kinds::String [StrandLike.uppercase];
    /// The words of the string.
    words(string: StrandLike) -> kinds::Array<kinds::String> [StrandLike.words];
    /// Whether the string has only letters.
    is_alpha(string: StrandLike) -> kinds::Bool [StrandLike.is_alpha];
    /// Whether the string has only letters and digits.
    is_alphanum(string: StrandLike) -> kinds::Bool [StrandLike.is_alphanum];
    /// Whether the string has only ASCII characters.
    is_ascii(string: StrandLike) -> kinds::Bool [StrandLike.is_ascii];
    /// Whether the string is a datetime, in `format` when given.
    is_datetime(string: StrandLike; format: StrandLike) -> kinds::Bool [StrandLike.is_datetime];
    /// Whether the string is a domain name.
    is_domain(string: StrandLike) -> kinds::Bool [StrandLike.is_domain];
    /// Whether the string is an email address.
    is_email(string: StrandLike) -> kinds::Bool [StrandLike.is_email];
    /// Whether the string is hexadecimal.
    is_hexadecimal(string: StrandLike) -> kinds::Bool [StrandLike.is_hexadecimal];
    /// Whether the string is an IP address.
    is_ip(string: StrandLike) -> kinds::Bool [StrandLike.is_ip];
    /// Whether the string is an IPv4 address.
    is_ipv4(string: StrandLike) -> kinds::Bool [StrandLike.is_ipv4];
    /// Whether the string is an IPv6 address.
    is_ipv6(string: StrandLike) -> kinds::Bool [StrandLike.is_ipv6];
    /// Whether the string is a latitude.
    is_latitude(string: StrandLike) -> kinds::Bool [StrandLike.is_latitude];
    /// Whether the string is a longitude.
    is_longitude(string: StrandLike) -> kinds::Bool [StrandLike.is_longitude];
    /// Whether the string has only digits.
    is_numeric(string: StrandLike) -> kinds::Bool [StrandLike.is_numeric];
    /// Whether the string is a record id, of `table` when given.
    is_record(string: StrandLike; table: ValueLike) -> kinds::Bool [StrandLike.is_record];
    /// Whether the string is a semantic version.
    is_semver(string: StrandLike) -> kinds::Bool [StrandLike.is_semver];
    /// Whether the string is a ULID.
    is_ulid(string: StrandLike) -> kinds::Bool [StrandLike.is_ulid];
    /// Whether the string is a URL.
    is_url(string: StrandLike) -> kinds::Bool [StrandLike.is_url];
    /// Whether the string is a UUID.
    is_uuid(string: StrandLike) -> kinds::Bool [StrandLike.is_uuid];
}

/// String distance functions, `string::distance::*`.
pub mod distance {
    use crate::kinds;

    functions! { $ "string::distance" "::", string_distance, string::distance;
        /// The Damerau–Levenshtein distance.
        damerau_levenshtein(a: StrandLike, b: StrandLike) -> kinds::Int
            [StrandLike.distance_damerau_levenshtein];
        /// The Hamming distance between two strings of equal length.
        hamming(a: StrandLike, b: StrandLike) -> kinds::Int [StrandLike.distance_hamming];
        /// The Levenshtein distance.
        levenshtein(a: StrandLike, b: StrandLike) -> kinds::Int
            [StrandLike.distance_levenshtein];
        /// The normalised Damerau–Levenshtein distance, from 0 to 1.
        normalized_damerau_levenshtein(a: StrandLike, b: StrandLike) -> kinds::Float
            [StrandLike.distance_normalized_damerau_levenshtein];
        /// The normalised Levenshtein distance, from 0 to 1.
        normalized_levenshtein(a: StrandLike, b: StrandLike) -> kinds::Float
            [StrandLike.distance_normalized_levenshtein];
        /// The optimal string alignment distance. SurrealDB has no method form of it.
        osa(a: StrandLike, b: StrandLike) -> kinds::Int;
    }
}

/// HTML functions, `string::html::*`.
pub mod html {
    use crate::kinds;

    functions! { $ "string::html" "::", string_html, string::html;
        /// The string with HTML special characters escaped.
        encode(string: StrandLike) -> kinds::String [StrandLike.html_encode];
        /// The string with unsafe HTML removed.
        sanitize(string: StrandLike) -> kinds::String [StrandLike.html_sanitize];
    }
}

/// String similarity functions, `string::similarity::*`.
pub mod similarity {
    use crate::kinds;

    functions! { $ "string::similarity" "::", string_similarity, string::similarity;
        /// The fuzzy match score; the same as `smithwaterman`.
        fuzzy(a: StrandLike, b: StrandLike) -> kinds::Int [StrandLike.similarity_fuzzy];
        /// The Jaro similarity, from 0 to 1.
        jaro(a: StrandLike, b: StrandLike) -> kinds::Float [StrandLike.similarity_jaro];
        /// The Jaro–Winkler similarity, from 0 to 1.
        jaro_winkler(a: StrandLike, b: StrandLike) -> kinds::Float
            [StrandLike.similarity_jaro_winkler];
        /// The Smith–Waterman fuzzy match score. Undocumented in 3.3.4 but still dispatched.
        smithwaterman(a: StrandLike, b: StrandLike) -> kinds::Int
            [StrandLike.similarity_smithwaterman];
        /// The Sørensen–Dice coefficient, from 0 to 1.
        sorensen_dice(a: StrandLike, b: StrandLike) -> kinds::Float
            [StrandLike.similarity_sorensen_dice];
    }
}

/// Semantic version functions, `string::semver::*`.
pub mod semver {
    use crate::kinds;

    functions! { $ "string::semver" "::", string_semver, string::semver;
        /// `-1`, `0` or `1` as `left` is lower than, equal to or higher than `right`.
        compare(left: StrandLike, right: StrandLike) -> kinds::Int [StrandLike.semver_compare];
        /// The major version.
        major(version: StrandLike) -> kinds::Int [StrandLike.semver_major];
        /// The minor version.
        minor(version: StrandLike) -> kinds::Int [StrandLike.semver_minor];
        /// The patch version.
        patch(version: StrandLike) -> kinds::Int [StrandLike.semver_patch];
    }

    /// Version increments, `string::semver::inc::*`.
    pub mod inc {
        use crate::kinds;

        functions! { $ "string::semver::inc" "::", string_semver_inc, string::semver::inc;
            /// The version with its major part incremented.
            major(version: StrandLike) -> kinds::String [StrandLike.semver_inc_major];
            /// The version with its minor part incremented.
            minor(version: StrandLike) -> kinds::String [StrandLike.semver_inc_minor];
            /// The version with its patch part incremented.
            patch(version: StrandLike) -> kinds::String [StrandLike.semver_inc_patch];
        }
    }

    /// Version updates, `string::semver::set::*`.
    pub mod set {
        use crate::kinds;

        functions! { $ "string::semver::set" "::", string_semver_set, string::semver::set;
            /// The version with its major part set to `value`.
            major(version: StrandLike, value: NumberLike) -> kinds::String
                [StrandLike.semver_set_major];
            /// The version with its minor part set to `value`.
            minor(version: StrandLike, value: NumberLike) -> kinds::String
                [StrandLike.semver_set_minor];
            /// The version with its patch part set to `value`.
            patch(version: StrandLike, value: NumberLike) -> kinds::String
                [StrandLike.semver_set_patch];
        }
    }
}
