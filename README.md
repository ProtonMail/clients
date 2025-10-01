# Proton Clients Monorepo

**Status**: WIP  
**Contact**: @nmarietta, @eackerma  
**Started**: June 2025  

This monorepo hosts client-side code for Proton applications (e.g. Mail, Calendar), shared SDKs, and supporting tools.

## Structure

```text
project/
  <project>/
    <platform>/         # android, apple, rust
      <module>/         # e.g. mail-composer, calendar-api
build/                  # scripts and tools
doc/                    # versioned ADRs, RFCs, guidelines
```

Each project is self-contained, and code is grouped by product first, then platform.

## Creating New Projects

To create a new project with the proper structure and GitLab CI configuration, use the `create_project.sh` script:

```bash
./create_project.sh <ProjectName>
```

This script will:
- Create the project directory structure (`project/<project>/android/`, `project/<project>/apple/`, `project/<project>/rust/`)
- Generate appropriate `.gitignore` files for each platform
- Create placeholder `README.md` files
- Set up the GitLab CI configuration file (`.gitlab-ci.yml`) for the project
- Automatically update the main `.gitlab-ci.yml` to include the new project

Example:
```bash
./create_project.sh Pass
# Creates project/pass/ with all necessary files and configuration
```

## Conventions

### Commits

You SHOULD use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

Your commit SHOULD:
* Use a proper sentence as a description — start with an Uppercase letter, end with a dot.
* Use the common commit types.
* Whenever possible use a project as a scope (e.g. `fix(mail,apple): Fixed schedule send delay.`).
* Remember: the commits will be used to generate changelog.

#### Allowed commit types

- `ci`: CI configuration.
- `chore`: No source or test files modified (e.g. tooling, script, dependency updates, maintenance).
- `doc`: Documentation changes.
- `feat`: A new feature.
- `fix`: Bug fixes.
- `i18n`: Internationalization and translations.
- `refactor`: A change in the source code that neither fixes a bug nor adds a feature.
- `revert`: Reverting a commit.
- `style`: Code style changes, not affecting code meaning (formatting).
- `test`: Adding new tests or improving existing ones.
- `perf`: Performance improvements, changes that make the code run faster, use less memory, etc. No functionality change.

### Branching

| Branch name | Pattern                  | Remarks                                                                   |
|-------------|--------------------------|---------------------------------------------------------------------------|
| main        | `main`                   | Main line branch, where we merge back, should always be production ready. |
| feature     | `<project>/feature/...`  |                                                                           |
| fix         | `<project>/fix/...`      |                                                                           |
| refactor    | `<project>/refactor/...` |                                                                           |
| release     | `<project>/release/...`  | See Releasing section below.                                              |

### Merging

When merging into `main` branch:
* You MUST rebase and fast-forward in order to keep the history linear.
* You MUST NOT use merge commits.
* You MUST only merge if the pipeline is successful, passing all minimal tests (described in the containing folder changes).
* You SHOULD run enough tests to be sure the MR is not breaking any tests in the monorepo.

### Reverting

When a merged commit break a test/project/platform/app, by default:
* The affected team SHOULD ask for a revert/rollback.
* The team owner of the breaking commit SHOULD take care of the revert process (e.g. MR, review, conflict, git revert).

### Releasing

You MUST create a release branch from the `main` branch, following this pattern:

`<project>/release/<platform>/.../<version>`

You MAY work on this branch for final release touch-ups. If you do, you SHOULD cherry-pick back to main.

Note: The preferred process is to fix the main branch, and then cherry-pick the commit in your release branch.

You MAY tag by project and module:  
Example: `@mail/android/mail-composer-1.0.2`

### Code Owners

There SHOULD be a CODEOWNERS file per directory that scopes the CODEOWNERS independently of the file structure.
That has the effect that CODEOWNERS will be enforced even if the directory is moved.

### CI/CD

* CI/CD triggers MUST be scoped per project with independent pipelines.
* Main branch changes trigger all minimal tests execution.
* Nightly tests might execute more than all minimal tests.
* The CI system MUST utilize the same commands employed by developers for building and testing purpose.
* Each project SHOULD provide a Gitlab CI pipeline yml file (.gitlab-ci.yml) with:
    * Minimal tests: Any change in a specific project will run at least this set of tests.
    * Manual tests: Any team should be able to manually run any tests.

## Android Development

### Build System Overview

The Android projects use **Gradle composite builds** to manage dependencies between modules across different projects. This enables sharing common modules (like `core/design-system`) across multiple Android applications while maintaining project separation.

#### Project Structure
```
project/
  core/android/                 # Shared modules
    design-system/              # UI components, themes, utilities
      test-fixtures/            # Test utilities (e.g. snapshot testing)
  account/android/              # Account-specific modules
    account-manager-ui/         # Account UI components
    app/                       # Account demo/test application
  <other-projects>/android/     # Additional Android projects
```

### Building Modules

Run Gradle commands from the monorepo root using the `-p` flag to specify the project directory:

```bash
# Build a specific module
./gradlew -p project/account/android :app:assembleDebug

# Run tests
./gradlew -p project/account/android :account-manager-ui:testDebugUnitTest
```

### Composite Build Configuration

Android projects that depend on core modules use **composite builds** with **dependency substitution**:

#### settings.gradle.kts Example
```kotlin
// Include the core Android project as a composite build
includeBuild("../../core/android") {
    dependencySubstitution {
        substitute(module("me.proton.core:design-system"))
            .using(project(":design-system"))
    }
}
```

#### Using Core Dependencies
Reference core modules in your `build.gradle.kts`:

```kotlin
dependencies {
    implementation(libs.proton.core.designsystem) // From version catalog
}
```

### Version Catalog

Shared dependencies are managed through a **centralized version catalog** (`gradle/libs.versions.toml`):

### Testing

#### Unit Tests

The project uses **JUnit5** for parameterized unit tests.

#### Snapshot Testing with Paparazzi
The project uses **JUnit4 with TestParameterInjector** for parameterized snapshot tests.

Generate and record visual snapshot tests:

```bash
# Record golden snapshots (update baseline images)
./gradlew -p project/account/android :account-manager-ui:recordPaparazziDebug
```

**Note**: Snapshot PNG files are tracked with **Git LFS** to avoid repository bloat.

## Open Source Mirror

A sanitized mirror will be published to:  
https://github.com/ProtonMail/clients
