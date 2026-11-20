# umbriel-tests

Every Umbriel test lives here. `tests/integration/main.rs` is the single integration test binary; `tests/compile.rs` runs the trybuild cases and the fixture crates.

No test starts a SurrealDB of its own. Tests that need a database run against servers you start, and each test works in a database of its own that it removes when it finishes, whether it passed or failed.

## Configuration

| Variable | Server |
|---|---|
| `UMBRIEL_TEST_URL` | WebSocket (`ws://` or `wss://`). Allows scripting, evaluated queries and the experimental `files` and `surrealism` features, keeps versions (SurrealKV with `versioned=true`), and has a temporary directory. |
| `UMBRIEL_TEST_RESTRICTED_URL` | WebSocket, with the server's default capabilities. Used by the tests that check a capability is refused when it is off. |
| `UMBRIEL_TEST_HTTP_URL` | HTTP (`http://` or `https://`). May be the same server as `UMBRIEL_TEST_URL`. |
| `UMBRIEL_TEST_USER`, `UMBRIEL_TEST_PASS` | A root user on every server above. |
| `UMBRIEL_TEST_MAPPER` | The absolute path, on the `UMBRIEL_TEST_URL` server, of an analyzer mapper file in a directory listed in that server's `SURREAL_FILE_ALLOWLIST` (`SURREAL_BUCKET_FOLDER_ALLOWLIST` covers buckets only), holding the one line `hi<TAB>hello` (a tab between the words). |

The `UMBRIEL_TEST_URL` server must also be built with SurrealDB's `cjk` feature, for the `segment` tokenizer, on a 64-bit, non-WebAssembly target, for DiskANN indexes.

A missing variable fails the tests that need it, naming the variable. Nothing is skipped.

## Test databases

Each test defines a database named `umbriel_test_<uuid>` in the namespace `umbriel_test`, and removes it by name when it finishes. The migration generator's tests define a namespace of their own, `umbriel_test_ns_<uuid>`, and remove it the same way. A removal that fails is reported with the statement that failed.

An interrupted run (a killed process, Ctrl-C) leaves these behind. List the databases with `INFO FOR NS;` in the `umbriel_test` namespace and the namespaces with `INFO FOR ROOT;`, and remove each by name with `REMOVE DATABASE <name>;` or `REMOVE NAMESPACE <name>;`. Nothing removes them automatically.

## Running

```sh
cargo test -p umbriel-tests
```

`cargo nextest run -p umbriel-tests` works too, if you have nextest installed.
