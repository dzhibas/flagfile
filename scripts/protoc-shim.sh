#!/bin/sh
# protoc shim for building `raft-proto` (via the old `protobuf-build 0.14.1`).
#
# `protobuf-build` only accepts a protoc whose *major* version is exactly `3`
# (see protobuf_impl.rs: `if major == 3 && minor >= 1`).
# Modern protoc reports e.g. "libprotoc 35.0", which
# that check rejects, breaking `cargo build`. It also ships no bundled protoc
# for macOS aarch64, so the fallback path panics too.
#
# This shim reports a compatible 3.x version for `--version` and delegates every
# other invocation to the real protoc found on PATH. Wired in via
# `.cargo/config.toml` (PROTOC env var). prost-build / tonic-build don't perform
# the broken major-version check, so faking the version is harmless for them.

if [ "$1" = "--version" ]; then
    echo "libprotoc 3.21.12"
    exit 0
fi

# Find the real protoc on PATH (this shim is referenced via $PROTOC, not PATH,
# so `command -v protoc` resolves to the genuine compiler).
real_protoc=$(command -v protoc)
if [ -z "$real_protoc" ]; then
    echo "protoc-shim: no real 'protoc' found on PATH" >&2
    exit 1
fi

exec "$real_protoc" "$@"
