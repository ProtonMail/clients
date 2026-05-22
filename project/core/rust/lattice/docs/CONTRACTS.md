# Lattice HTTP Contracts — Contributor Guide

This guide outlines the rules for adding or changing **SlimAPI HTTP contracts** in the `lattice` crate.

## Transport (Muon) integration

The core **`lattice`** crate defines contracts and, with the **`serde`** feature, the transport-neutral wire layer (`lattice::transport`). It has **no** `muon` / `mail-muon` dependencies. HTTP is wired through two adapter crates:

| Layer | Role |
|-------|------|
| **`lattice::transport`** (feature `serde`) | Wire types (`LtWireRequest`, `LtWireResponse`) and [`LtTransportProvider`](../src/transport/provider.rs) (contract → wire → send → parse). See `cargo doc -p lattice --features serde --open`. |
| **`lattice-muon1`** | Mail-muon v1 adapter: [`Muon1Transport`](../../lattice-muon1/src/transport.rs), [`Muon1WireRequestProvider`](../../lattice-muon1/src/wire.rs), [`LatticeExt`](../../lattice-muon1/src/ext.rs), [`RunLatticeContractExt`](../../lattice-muon1/src/ext.rs). |
| **`lattice-muon2`** | Muon v2 adapter: [`Muon2Transport`](../../lattice-muon2/src/transport.rs), [`Muon2WireRequestProvider`](../../lattice-muon2/src/wire.rs), [`LatticeExt`](../../lattice-muon2/src/ext.rs). |

**SlimAPI contracts** — call [`LtTransportProvider::send_contract_request`](../src/transport/provider.rs) (or `LatticeExt::send_with` on the muon crate). Response parsing uses [`LtWireResponse::into_contract_response`](../src/transport/wire_response.rs) (`T::Response::from_body` for success, `LtApiResponseError` for 4xx).

**Errors** — muon crates expose [`LtTransportError`](../../lattice-muon2/src/error.rs) (`Transport` \| `Lattice`). Keep HTTP/network failures on `LtTransportError::Transport`; do not map them to `LatticeError::Other`.

**Muon v1 (mail stack)** — prefer [`mail-api-lattice`](../../../mail/rust/api/mail-api-lattice/src/lib.rs) for `ApiServiceError` mapping; it uses `Muon1Transport` and defines its own `RunLatticeContractExt` with `run_lattice_contract` → `LtTransportError` and `run_lattice_contract_compat` → `ApiServiceError`. Alternatively use `lattice-muon1` directly.

**Muon v2** — `LatticeExt::send_with(session)` builds a [`Muon2Transport`](../../lattice-muon2/src/transport.rs) and calls `send_contract_request`.

**Quark (optional)** — Quark command types live in the separate [`lattice-quark`](../../lattice-quark/) crate (`lattice_quark::user`, `lattice_quark::payments`, …). Extension traits encode contracts to [`LtWireRequest`](../../lattice/src/transport/wire_request.rs) ([`LtQuarkWireExt::to_wire_request`](../../lattice-quark/src/transport/lt_quark_wire_ext.rs)) and parse [`LtWireResponse`](../../lattice/src/transport/wire_response.rs) ([`LtQuarkResponseExt::into_quark_response`](../../lattice-quark/src/transport/lt_quark_response_ext.rs)). Send via [`LtQuarkTransportProvider::send_contract_quark`](../../lattice-quark/src/transport/lt_quark_transport_provider.rs) on any [`LtTransportProvider`](../src/transport/provider.rs) (e.g. `Muon2Transport` from `lattice-muon2`). Add `lattice-quark` and a muon adapter in dev-dependencies; muon crates have no `quark` feature.

**Wire sensitivity** — On the transport wire layer, header values, query values, and HTTP bodies use [`Sensitive`](../../lattice/src/sensitive.rs). Muon adapters ([`Muon1WireRequestProvider`](../../lattice-muon1/src/wire.rs), [`Muon2WireRequestProvider`](../../lattice-muon2/src/wire.rs)) unwrap with `into_inner()` only when building native `HttpReq` / reading native `HttpRes`. Contract `headers()` should return `HashMap<String, Sensitive<String>>` (same as query params).

---

## 1. The "Golden" Example

Before diving into the rules, here is a complete, well-formed example of a standard POST request contract. Notice how **path parameters** and **JSON bodies** are cleanly separated.

```rust
use std::borrow::Cow;
use crate::contract::{LtContract, Method, LtSlimAPIJSON, AuthReq};

// 1. Request Type (Holds path/query params and links to the body payload)
#[derive(Debug)]
pub struct LtCorePostDomainReq {
    pub domain_id: String, 
    pub body: LtCorePostDomainBody,
}

// 2. Request Body DTO (Only the JSON payload)
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct LtCorePostDomainBody {
    #[cfg_attr(feature = "serde", serde(rename = "DomainName"))]
    pub name: String,
}

// 3. Response DTO (Associated strictly with this request)
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct LtCorePostDomainRes {
    pub success: bool,
}

// 4. Contract Implementation
impl LtContract for LtCorePostDomainReq {
    type Response = LtSlimAPIJSON<LtCorePostDomainRes>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCorePostDomainBody>;

    fn path(&self) -> Cow<'_, str> {
        Cow::Owned(format!("/core/v4/domains/{}", self.domain_id))
    }

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, crate::LatticeError> {
        // Pass only the body struct to the JSON wrapper
        Ok(Method::Post(LtSlimAPIJSON(&self.body)))
    }
}

// 5. Authentication Marker
impl AuthReq for LtCorePostDomainReq {}
```

---

## 2. Architecture, Naming, & File Layout

Lattice categorizes API endpoints using a strict hierarchy based on the API documentation: **API Sections** and **Domains**.

* **API Section:** The top-level namespace of the API (e.g., `core`, `auth`, `drive`). This corresponds to the root folder (e.g., `src/core/`). Note that each section is under its own feature flags, so avoid cross-section dependencies or add dependencies between the flags in the `Cargo.toml`.
* **Domain:** The specific resource category as defined in the API Doc (e.g., `OrganizationInviteToken`, `Authentication Sessions`). This corresponds to a subfolder (e.g., `src/core/organization_invite_token/`).

### 2.1 Type Naming Rules
* **Format:** `Lt[Section][Request/response method][Domain/Resource][Verb]Req` (or `Res` / `Body`).
* **Example:** A POST request to create an authentication session in the `auth` section becomes `LtAuthPostAuthenticationSessionReq`. Avoid inconsistent abbreviations.

### 2.2 File Layout & Common Types
* **File Names:** Mirror the verb and route shape (e.g., `get_domain.rs`, `post_domains.rs`) inside the appropriate `src/[section]/[domain]/` folder.
* **Strict File Isolation:** A request file must **only** contain types directly associated with that specific request (the `Req`, its specific `Body`, and its specific `Res`). **Do not put non-request-associated types in a request file.**
* **Common Types:**
  * If a type is shared across a **Domain**, put it in `src/[section]/[domain]/mod.rs`.
  * If a type is shared across an entire **API Section**, put it in `src/[section]/mod.rs`.

**File Layout Example:**
```text
src/core/                                // <-- API Section (`core`)
├── mod.rs                               // Put core-wide common types here
└── organization_invite_token/           // <-- Domain
    ├── mod.rs                           // Put domain-wide common types here
    └── post_organization_invite.rs      // Request-specific types only!
```

**Section Checklist:**
- [ ] File is correctly placed in `src/[section]/[domain]/[verb]_[route].rs`.
- [ ] Types follow the `Lt[Section][Request/response method][Domain][Verb]Req` naming convention.
- [ ] **No shared/common types** are declared inside the request file.
- [ ] Shared types are placed in the appropriate `mod.rs`.

---

## 3. Implementing `LtContract`

Always implement `LtContract` directly on the **request struct** (the value the client holds), not on a separate "builder" type. 

### 3.1 Separating Path Parameters and Bodies
* **Do not mix path fields and body fields into the same struct.** * Instead, place path parameters (e.g., `domain_id`) directly on the `Req` struct. 
* Create a separate `Body` type for the JSON payload, and link it via a `pub body: YourBodyType` field on the `Req` struct.

### 3.2 Methods & Return Types
* **Responses:** Use `type Response = LtSlimAPIJSON<T>` for all SlimAPI endpoints. Use `LtSlimAPIJSON<()>` if the response body is empty.
* **GET / DELETE Bodies:** Use `type Body<'a> = LtSlimAPIJSON<()>`.
* **POST / PUT Bodies:** Use `type Body<'a> = LtSlimAPIJSON<&'a YourBodyType>` and return `Ok(Method::Post(LtSlimAPIJSON(&self.body)))`.

### 3.3 Paths, Queries & Headers
* **Fixed Paths:** Return `Cow::Borrowed("/core/v4/domains")`. 
* **Dynamic Paths:** Return `Cow::Owned(format!("/core/v4/domains/{}", self.domain_id))`. 
* **Queries:** Return `Some(HashMap<String, String>)` using the casing the API expects (often **PascalCase**). Do not hardcode queries into the path string.
* **Headers:** Default is empty. Only implement `headers()` if the spec explicitly requires custom headers (e.g., `X-PM-*`).

**Section Checklist:**
- [ ] `LtContract` is implemented on the request struct.
- [ ] Path parameters and body fields are strictly separated into different structs.
- [ ] `Response` and `Body` types correctly use `LtSlimAPIJSON`.
- [ ] Path parameters are handled via `Cow::Owned(format!(...))`.

---

## 4. Authentication Markers

Every contract must explicitly declare its authentication requirements.

**Rules:**
* Add either `impl AuthReq for YourReq {}` or `impl UnauthReq for YourReq {}`.
* Place this marker on the **request type** right below the `LtContract` implementation.

**Section Checklist:**
- [ ] `AuthReq` or `UnauthReq` trait is implemented on the request struct.

---

## 5. Serde (JSON) & Secrets

We gate Serde derives behind `#[cfg_attr(feature = "serde", ...)]` to allow the crate to compile faster for consumers who don't need serialization.

**Rules:**
* **PascalCase:** Use `serde(rename_all = "PascalCase")` on DTOs if the API uses PascalCase. Provide per-field overrides for weird acronyms (e.g., `serde(rename = "AllowedForSSO")`).
* **Optionals:** Use `serde(skip_serializing_if = "Option::is_none")` to omit absent keys instead of sending `null`.
* **Unknown Fields:** Use `serde(deny_unknown_fields)` on all types but ensure if it gated behind `test` and `serde` so it only runs in tests.
* **Wrapper types:** Use wrapper types around `Strings`, `numbers`... When possible to make code more expressive on IDs.
* **Enum types:** Prefer using enums then numbers when possible.
* **Boolean types:** Often in the API `i32` are used in place of headers. Use the `bool_int` serde helper for this.
* **Bitflags types:** Other times `i32` are for bitflag types use the `bitflag` for this.
* **Multiple possible outputs:** If a type changes multiple optional fields depending on it's presence use untagged enums.
* **Secrets:** Use `Sensitive<T>` for tokens, keys, and passwords.

**Section Checklist:**
- [ ] `cfg_attr` is used for all Serde derives.
- [ ] Secrets use the `Sensitive<T>` wrapper.
- [ ] `deny_unknown_fields` is strictly avoided on production code.

---

## 6. Edge Cases

### Non-SlimAPI Success Bodies (e.g., HTML)
`LtSlimAPIJSON<T>` expects a specific SlimAPI JSON envelope. It cannot parse HTML or raw text.
* **Rule:** Implement `LtResponseBody` for a dedicated response type, set `type Response = ThatType`, and leave a **module-level comment** explaining why it deviates from `LtSlimAPIJSON`.

### Plain JSON Endpoints
Some legacy or third-party endpoints speak plain JSON without the SlimAPI `code` + `body` wrapper.
* **Rule:** Use `LtJson<T>` only for these specific endpoints. If unsure, check the OpenAPI spec or backend implementation.

**Section Checklist:**
- [ ] If returning HTML/Plain Text, a custom `LtResponseBody` is implemented and documented.
- [ ] `LtJson` is only used for strictly non-SlimAPI endpoints.

---

## 7. Integration tests — accounts and credentials

Integration tests under `tests/` must **not** rely on **hardcoded usernames or passwords**. Accounts used in tests must be **created through Quark** (or equivalent provisioning exposed by the test harness) so runs are isolated and do not assume a pre-seeded environment.

**Do**

* Add **`lattice-quark`** and **`lattice-muon2`** (or `lattice-muon1`) in dev-dependencies when tests send Quark commands.
* Create users with **`LtQuarkContract`** types from `lattice_quark` (e.g. `LtQuarkUserCreate`, `LtQuarkUserCreateOrganization`, …) via `LtQuarkTransportProvider::send_contract_quark` / `send_quark` helpers in `tests/common/`.
* Derive **unique** credentials per run using helpers such as **`random_username()`**, **`random_password()`**, and related utilities in `tests/common` when the test needs a name or secret string.
* Thread the **returned** username/password from Quark responses into login and follow-up API calls.

**Don’t**

* Embed literal strings like `"testuser"` / `"password123"` (or copy-pasted production-like credentials) as the primary test identity.
* Assume a fixed account already exists on the target stack unless the test is explicitly documented as requiring external setup (rare; prefer Quark).

**Section checklist**

- [ ] No hardcoded username/password for the main account under test.
- [ ] User/org/domain setup goes through **Quark** (or documented harness) with generated or response-derived credentials.

---

## 8. SlimAPI errors (`LtApiResponseError`)

When the backend returns a **new** documented SlimAPI error code that tests or clients must distinguish, extend the lattice error model in line with existing patterns.

**Do**

* Add a variant to **`LtApiResponseError`** in `src/errors/mod.rs` with a stable **`#[display("...")]`** name that matches how the API surfaces the error.
* Use **`EnforcedCode<{numeric_code}>`** when the code is fixed by the API contract; use **`NullErrorDetails`** when there is no structured `details` payload, or add a **details struct** under `src/details/` (and wire it in the variant) when the response includes typed fields.
* Keep **`serde`** attributes on `LtApiResponseError` and related types aligned with the SlimAPI JSON shape (`PascalCase` / `untagged` as already used in that enum).
* Add or update **integration tests** that assert the new variant (e.g. `assert_api_err!` / pattern match on `LtApiResponseError::YourVariant`) when behavior is contract-critical.

**Don’t**

* Map the same API code to multiple variants without a strong reason; prefer **one variant per distinct code** (or one variant with shared details type) unless the backend truly overloads one code.
* Add error variants for **transient** or undocumented codes without backend confirmation — use the existing **`Other`** arm when deserialization must remain permissive.

**Section checklist**

- [ ] New API error codes have a corresponding **`LtApiResponseError`** variant (or are intentionally handled via **`Other`**).
- [ ] Details types live under `src/details/` when the wire format carries structured fields.
- [ ] Tests cover at least one assertion path for new error behavior where feasible.

---

## 9. Final PR Checklist

Before submitting your PR, verify the following:

- [ ] I have reviewed the "Golden Example" and my contract matches its separated path/body structure.
- [ ] No non-request associated types are lingering in my request file (they are moved to `mod.rs`).
- [ ] `type Response = LtSlimAPIJSON<...>` is used (unless dealing with a documented edge case).
- [ ] `AuthReq` or `UnauthReq` is implemented.
- [ ] New modules are properly wired in `mod.rs` with explicit `pub use`.
- [ ] **Tests:** no hardcoded primary username/password; Quark-backed provisioning where integration tests need accounts.
- [ ] **Errors:** new SlimAPI codes are reflected in `LtApiResponseError` / `details` as appropriate.