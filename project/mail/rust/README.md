# Proton Rust libraries

This repo maps part of the Core proton REST api. It's designed so that each team can extend this
with their own primitives, requests and domain types as needed.

## Style

All Rust code must be formatted with `cargo fmt` before committing.

All TOML files must be formatted with `taplo fmt`. You can install the Taplo CLI tool with:

```bash
cargo install taplo-cli --locked
```

Any code not formatted this way will be rejected by the CI.

## Bazel

Build the full mail Rust graph:

```bash
bazel build //project/mail/rust/...
```

### Features and dependencies

Bazel does not propagate `crate_features` to dependencies the way Cargo does (`dep/feature` entries in
`Cargo.toml`). A `crate_features` list on a `rust_library` only enables `cfg(feature = "...")` in
that crate's own sources.

For the default mail production graph, set features on each dependency's `BUILD.bazel` target
explicitly (for example `sql` on `mail-api`). Use the crate's `Cargo.toml` `[features]` section as
the reference for which downstream crates need which features.

Run Bazel tests locally:

```bash
bazel test //project/mail/rust/...
```

## CI

On merged-result / merge-train pipelines, when mail Rust paths change (see
`project/mail/.gitlab-ci.yml`):

| Job | Command | Notes |
|-----|---------|-------|
| `test:linux` | `bazel test //project/mail/rust/...` (via `ci/project.gitlab-ci.yml`) | Monorepo-wide Linux Bazel tests |
| `mail:clippy` | `bazel build --config=clippy //project/mail/rust/...` | Linux |
| `mail:rust:test:macos` | `bazel test --config=mail-darwin-test //project/mail/rust/...` | macOS (tart runner); mirrors Cargo `mail-darwin-test` profile |
| `mail:build-mail-uniffi-ios` | `bazel build //project/mail/apple/mail-uniffi:ProtonAppUniffi` | UniFFI / release tag |
| `mail:build-mail-uniffi-android` | `bazel build :mail_uniffi_android_jni_libs` + Gradle archive | UniFFI / release tag |
| `mail:deny` / `mail:gopenpgp` | `cargo deny` / `cargo tree` | Workspace policy checks |

Formatting is enforced by the monorepo-wide `lint` job (`bazel run //:format -- check`),
including mail `Cargo.toml` files.

Bazel test targets mirror Cargo's three layers where they exist:

- `rust_test` — unit tests compiled from `#[cfg(test)]` modules in `src/`
- `rust_test_suite` / `rust_test` + `crate_root` — integration tests under `tests/`
- `rust_doc_test` — documentation examples

Crates with no unit tests omit the `rust_test` stub. Dev binaries and fuzz targets (for example
`mail-tui`, `mail-ical-cli`, `*-fuzz`, `mail-uniffi-bindgen`) stay tagged `manual` and are
excluded from `bazel test //project/mail/rust/...`.

`test:linux` (monorepo root) and `mail:rust:test:macos` run unit, doc, and integration tests that
build under Bazel today (including `mail-action-queue-integration-tests` and
`mail-core-common-integration-tests`). Suites that still need Bazel `test-utils` / `mocks` wiring
(`mail-common`, `mail-api`, `mail-calendar-common`, and some `test-utils` unit tests) stay
`manual` until [!3085](https://gitlab.protontech.ch/proton/clients/monorepo/-/merge_requests/3085).

## Releases

### Conventions

When your product/crate is ready for a release, create a new branch with the following syntax:

- `releases/$PRODUCT/$MAJOR.$MINOR`

When you are ready to release create a tag with the following syntax:

- `$PRODUCT-v$MAJOR.$MINOR.$PATCH`

For instance for the `mail-uniffi` crate this would translate into:

- Branch: `releases/mail-uniffi/0.55`
- Tag: `mail-uniffi/0.55`

### Procedure of the release

#### New release

- Bump version in **master**.
- Run the script for generating changelog.
- Create the respective tag an push it.
- Create the respective branch and push it.
- Notify Slack channel about the pipeline with gist from the changelog.

#### Fix Releases

- Bump version in the respective **Release Branch**
- Run the script for generating changelog.
- Create the tag and push it.
- Merge branch back to `master` (but do not delete source branch!)
- Notify Slack channel about the pipeline with gist from the changelog.

### Guidelines

- If you are working on bug fix or improvement directly related to the release, create a merge
  request targeting the release branch and then port the release to master.
- Backporting fixes from master should be avoided if possible as you are likely to pull in new
  unrelated changes. This may not be avoidable in all cases and it may be better to create a new
  release if the merge conflicts are too great.

### Generating the Changelog

You should use the `./scripts/changelog` tool to generate the Changelog.

This can be invoked with the prepared script:

```bash
$ pipx install uv # if needed
$ sh ./mail/mail-uniffi/scripts/gen_changelog.sh
```

To skip commits from the changelog you should add an `*` before the `:` in the commit message:

```
feat*: this will not be in the changelog
```

```
feat(ET-1234)*: This will also not be in the changelog
```

## Nix

If you're using Nix, you can use devenv to pull most of our dependencies - just
[install devenv](https://devenv.sh/getting-started) and run `devenv shell` to
get a shell with (mostly) everything in scope.

If you're not a fan of Nix, you don't have to install it, this is optional - you
will have to install dependencies (e.g. Go) by hand in this case, though.

Note that for building mail<->ios specific stuff you'll also need to provide a
custom ENV variables - create a file called `devenv.local.nix` with:

```nix
{ ... }:

{
  env.IOS_REPO_ROOT="<path to your ET apple inbox repository>";
}
```


Having that, use the `proton-build-ios` command to build the iOS stuff.

## Building xcode project & running app in simulator

First, you need to choose which version of iPhone simulator you would like to use.
You can see all available options by running:

```sh
xcrun simctl list
```

Example output:

```
...
-- iOS 18.3 --
    iPhone 16 Pro (918F79B8-70DC-4567-B0C6-6253B0D49C25) (Shutdown)
    iPhone 16 Pro Max (7C1E9F4F-38BF-4D70-9DA6-52CFF959C061) (Shutdown)
...
```

Save the UUID of chosen model to the env variable `DEVICE_ID`

`.envrc`:
```sh
export DEVICE_ID="7C1E9F4F-38BF-4D70-9DA6-52CFF959C06";
```

or `devenv.local.nix` if you use Nix:
```nix
env.DEVICE_ID = "7C1E9F4F-38BF-4D70-9DA6-52CFF959C061";
```

Then you can build and run the XCodeproj by invoking from the root folder:
```sh
./mail/mail-uniffi/ios/run-local.sh
```

(Or you can use `proton-run-ios` if you use Nix, which also enables you to run this command from any descendant folder :))

So in the end the process looks like this:

```sh
./mail/mail-uniffi/ios/build-local.sh
./mail/mail-uniffi/ios/run-local.sh

```

## Accessing logs from the simulator

You can run to get Rust logs:

```sh
xcrun simctl spawn "$DEVICE_ID" log stream \
      --predicate 'subsystem == "ch.protonmail.protonmail" \
      AND category == "[Proton] Rust"' \
      --style syslog
```

(Or you can use `proton-logs-ios` if you use Nix :))


If you need **all** logs, not just Rust one:

```sh
xcrun simctl spawn "$DEVICE_ID" log stream \
      --predicate 'subsystem == "ch.protonmail.protonmail" \
      --style syslog
```

## Building Platform Frameworks

For building iOS and Android frameworks (XCFramework/AAR), see:
- [rust-build/README.md](rust-build/README.md) - Build scripts documentation and profile comparisons

## Vendoring

To regenerate the `3rdparty` directory, use

```
cargo vendor --versioned-dirs --locked 3rdparty
```

