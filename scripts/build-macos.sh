#!/bin/bash
TARGET="flowsurface"
VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)
ARCH="universal"
RELEASE_DIR="target/release"
ARCHIVE_NAME="$TARGET-$ARCH-macos.tar.gz"
ARCHIVE_PATH="$RELEASE_DIR/$ARCHIVE_NAME"

# build binaries
export MACOSX_DEPLOYMENT_TARGET="11.0"
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
cargo build --release --target=x86_64-apple-darwin
cargo build --release --target=aarch64-apple-darwin

# create universal binary
mkdir -p "$RELEASE_DIR"
lipo "target/x86_64-apple-darwin/release/$TARGET" "target/aarch64-apple-darwin/release/$TARGET" -create -output "$RELEASE_DIR/$TARGET"

# create tar archive
tar -czf "$ARCHIVE_PATH" -C "$RELEASE_DIR" "$TARGET"

echo "Created archive at $ARCHIVE_PATH"