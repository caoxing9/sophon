#!/bin/sh
# Builds sophon-relay for x86-64 Linux as one static binary (musl), in
# Docker: relay/dist/sophon-relay. Runs on any Linux server, Ubuntu included.
set -eu
cd "$(dirname "$0")"
mkdir -p dist
docker run --rm --platform linux/amd64 \
  -v "$PWD":/src -w /src \
  -v sophon-relay-cargo:/usr/local/cargo/registry \
  rust:1-alpine sh -c 'apk add --no-cache musl-dev >/dev/null && cargo build --release --target-dir /tmp/target && cp /tmp/target/release/sophon-relay dist/'
ls -la dist/sophon-relay
