#!/usr/bin/env sh
# Native Linux launcher. Keep Cargo data/build output outside the DOS originals.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if ! command -v cargo >/dev/null 2>&1; then
    export CARGO_HOME="$project_dir/reverse/tools/cargo-linux"
    export RUSTUP_HOME="$project_dir/reverse/tools/rustup-linux"
    export PATH="$CARGO_HOME/bin:$PATH"
fi
build_dir=${CARGO_TARGET_DIR:-"$project_dir/stunts-rs/target"}
cargo build --release --manifest-path "$project_dir/stunts-rs/Cargo.toml" --features modern --bin stunts-modern --target-dir "$build_dir"
exec "$build_dir/release/stunts-modern" --data "$project_dir/stunts_dos" "$@"
