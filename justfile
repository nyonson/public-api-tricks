set quiet := true

export RBMT_LOG_LEVEL := env("RBMT_LOG_LEVEL", "progress")

project := file_name(justfile_directory())
rbmt_version := `grep "^rbmt.version" Cargo.toml | cut -d'"' -f2`

_default:
  just --list

# Install workspace dev tools
tools:
  echo "{{project}} dev tools [cargo-rbmt@{{rbmt_version}}]"
  cargo install --quiet cargo-rbmt --version {{rbmt_version}}
  cargo rbmt toolchains

# Run cargo-rbmt with given args
rbmt *args: tools
  cargo rbmt {{args}}

# Update minimal and maximum lockfiles
lock: (rbmt "lock --lockfiles minimal,maximum")

# Check docs
docs: (rbmt "docs --lockfile maximum")

# Format workspace
fmt: (rbmt "fmt")

# Check workspace lints
lint: (rbmt "lint --lockfile maximum")

# Test workspace
test: (rbmt "test --lockfile minimal --toolchain nightly")

# Check prerelease
prerelease: (rbmt "prerelease --force")

# Re-bless the API snapshots
bless: tools
  UPDATE_SNAPSHOTS=1 cargo rbmt run --lockfile minimal --toolchain nightly -- test
