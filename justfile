# Display available commands (default target)
default:
    @just --list


## Format

# Format the code (cargo fmt --all)
[group: 'format']
fmt:
    cargo +nightly fmt --all

# Check the code format (cargo fmt --check)
[group: 'format']
fmt-check:
    cargo +nightly fmt --all -- --check


## Check

# Neither recipe hardcodes --all-targets: the caller picks the configuration, and on
# a bare-metal target the test and bench targets cannot build at all, because they
# link the libtest harness and need a panic handler. The no_std build is
# `just check --no-default-features --target aarch64-unknown-none`.

# Compile check (cargo check)
[group: 'check']
check *EXTRA_FLAGS:
    cargo check {{EXTRA_FLAGS}}

# Lint (cargo clippy)
[group: 'check']
clippy *EXTRA_FLAGS:
    cargo clippy {{EXTRA_FLAGS}}

# Check for unused dependencies (cargo machete)
[group: 'check']
check-unused-deps:
    cargo machete


## Testing

# Run all tests
[group: 'test']
test *EXTRA_FLAGS:
    #!/usr/bin/env bash
    set -e
    if command -v "cargo-nextest" &> /dev/null; then
        cargo nextest run {{EXTRA_FLAGS}}
    else
        >&2 echo "================================================================="
        >&2 echo "WARNING: cargo-nextest not found - using 'cargo test' fallback"
        >&2 echo ""
        >&2 echo "For faster test execution, consider installing cargo-nextest:"
        >&2 echo "  cargo install --locked cargo-nextest@^0.9"
        >&2 echo "================================================================="
        sleep 1
        cargo test {{EXTRA_FLAGS}}
    fi


## Clean

# Clean build artifacts (cargo clean)
[group: 'clean']
clean:
    cargo clean


## Misc

PRECOMMIT_CONFIG := ".github/pre-commit-config.yaml"
PRECOMMIT_DEFAULT_HOOKS := "pre-commit pre-push"

# Install Git hooks
[group: 'misc']
install-git-hooks HOOKS=PRECOMMIT_DEFAULT_HOOKS:
    #!/usr/bin/env bash
    set -e # Exit on error

    # Check if pre-commit is installed
    if ! command -v "pre-commit" &> /dev/null; then
        >&2 echo "=============================================================="
        >&2 echo "Required command 'pre-commit' not available ❌"
        >&2 echo ""
        >&2 echo "Please install pre-commit using your preferred package manager"
        >&2 echo "  pip install pre-commit"
        >&2 echo "  pacman -S pre-commit"
        >&2 echo "  apt-get install pre-commit"
        >&2 echo "  brew install pre-commit"
        >&2 echo "=============================================================="
        exit 1
    fi

    # Install all Git hooks (see PRECOMMIT_DEFAULT_HOOKS for default hooks)
    pre-commit install --config {{PRECOMMIT_CONFIG}} {{replace_regex(HOOKS, "\\s*([a-z-]+)\\s*", "--hook-type $1 ")}}

# Remove Git hooks
[group: 'misc']
remove-git-hooks HOOKS=PRECOMMIT_DEFAULT_HOOKS:
    #!/usr/bin/env bash
    set -e # Exit on error

    # Check if pre-commit is installed
    if ! command -v "pre-commit" &> /dev/null; then
        >&2 echo "=============================================================="
        >&2 echo "Required command 'pre-commit' not available ❌"
        >&2 echo ""
        >&2 echo "Please install pre-commit using your preferred package manager"
        >&2 echo "  pip install pre-commit"
        >&2 echo "  pacman -S pre-commit"
        >&2 echo "  apt-get install pre-commit"
        >&2 echo "  brew install pre-commit"
        >&2 echo "=============================================================="
        exit 1
    fi

    # Remove all Git hooks (see PRECOMMIT_DEFAULT_HOOKS for default hooks)
    pre-commit uninstall --config {{PRECOMMIT_CONFIG}} {{replace_regex(HOOKS, "\\s*([a-z-]+)\\s*", "--hook-type $1 ")}}

# Install cargo-machete (unused dependency checker)
[group: 'misc']
install-cargo-machete:
    cargo install --locked cargo-machete

# Install cargo-nextest (faster test runner)
[group: 'misc']
install-cargo-nextest:
    cargo install --locked cargo-nextest
