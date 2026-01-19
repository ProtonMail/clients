# TCL (Telemetry Client Library)

## Overview

**TCL** is a cross-platform telemetry library that:

- Collects telemetry events (metrics with dimensions/values), documentation [here](https://gitlab.protontech.ch/data/engineering/telemetry/-/tree/master/definitions)
- Stores them locally in a database
- Syncs them to Proton's backend

## Platform Support

The library targets **4 different platforms**:

- **Native** (Linux/Mac/Windows)
- **WebAssembly** (Web browsers)
- **Android**/**iOS** (via uniffi)

:warning: The `sqlite` feature cannot be used on WASM (doesn't compile)

## Thread Safety

- The built-in `TelemetryHttpClient` and `SqliteDatabase` is thread-safe (uses `Mutex` internally)
- Concurrent `publish_events` calls: The `Tcl` struct does **not** synchronize `publish_events`. Calling it concurrently from multiple tasks can cause duplicate events to be sent (both tassks fetch the same events before either deletes them).

To prevent this, a custom implementations should serialize `publish_events` calls or only call from a single task

- `store_events`: Safe to call concurrently with other `store_events` or `publish_events` calls.
- WASM: Thread safe since WASM is single-threaded.

## Features

### Cargo Features

The library uses conditional compilation via cargo features:

#### `sqlite`

- Enables local SQLite database storage
- Includes Sqlite database implementation
- **Used on**: Native platforms (Android, iOS, Desktop)
- **NOT used on**: WASM

#### `http`

- Enables built-in HTTP client (`reqwest`)
- Sends telemetry to Proton's backend
- **Used on**: All platforms

#### `uniffi`

- Enables FFI bindings for Android and iOS
- **Used on**: Android and iOS

## Usage Examples

### Rust (Native) - Built-in HTTP and SQLite

```toml
[dependencies]
tcl = { git = "git+ssh://git@gitlab.protontech.ch/proton/clients/monorepo.git", features = ["http", "sqlite"] }
```

```rust
use core_telemetry::storage::SqliteDatabase;
use core_telemetry::http::TelemetryHttpClient;
use core_telemetry::Tcl;

let http = TelemetryHttpClient::new();
let db = SqliteDatabase::new("/tmp/telemetry.db")?;

// With references - can reuse http/db or create multiple Tcl instances
let tcl = Tcl::new(&http, &db);
tcl.store_events(vec![event]).await?;
tcl.publish_events(100).await?;

// Or with owned values - takes ownership of http/db
let tcl = Tcl::new(http, db);
```

### Rust (Native) - Bring-your-own HTTP Client

```toml
[dependencies]
tcl = { git = "git+ssh://git@gitlab.protontech.ch/proton/clients/monorepo.git", features = ["sqlite"] }
```

```rust
use core_telemetry::storage::SqliteDatabase;
use core_telemetry::{Tcl, TelemetryHttpClientEx};

// Provide your own HTTP client implementation
struct MyHttpClient { /* ... */ }
impl TelemetryHttpClientEx for MyHttpClient { /* ... */ }

let http = MyHttpClient::new();
let db = SqliteDatabase::new("/tmp/telemetry.db")?;
let tcl = Tcl::new(&http, &db);
```

### WASM - Bring-your-own Database

```toml
[dependencies]
tcl = { git = "git+ssh://git@gitlab.protontech.ch/proton/clients/monorepo.git", features = ["http"] }
```

```rust
use core_telemetry::http::TelemetryHttpClient;
use core_telemetry::{Tcl, TelemetryDbEx};

// Provide IndexedDB wrapper (WASM has no filesystem)
struct IndexedDbWrapper { /* ... */ }
impl TelemetryDbEx for IndexedDbWrapper { /* ... */ }

let http = TelemetryHttpClient::new();
let db = IndexedDbWrapper::new();
let tcl = Tcl::new(&http, &db);

tcl.store_events(vec![event]).await?;
tcl.publish_events(100).await?;
```

**Note**: WASM has unique constraints:

- Single-threaded (JavaScript event loop)
- No filesystem (can't use SQLite)
- async functions can't be `Send`

### Android/iOS - via UniFFI

```kotlin
val tcl = TclFfi("/data/app/telemetry.db")
tcl.storeEvents(listOf(event))
tcl.publishEvents(10u)
```

## Build Commands Reference

```bash
# Native (desktop) - full features
cargo build --features "sqlite,http"

# WASM (web) - HTTP only, custom DB
cargo build --target wasm32-unknown-unknown --features "http"

# Android/iOS - everything + uniffi
cargo build --features "uniffi"

# Run tests (native with all features)
cargo test --features "http,sqlite" -- --nocapture
```
