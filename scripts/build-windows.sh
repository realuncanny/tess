#!/bin/bash
EXE_NAME="flowsurface.exe"
TARGET="x86_64-pc-windows-msvc"
VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)

# update package version on Cargo.toml
cargo install cargo-edit
cargo set-version $VERSION

# build binary
rustup target add $TARGET
cargo build --release --target=$TARGET

# create staging directory
mkdir -p target/release/win-portable

# copy executable and assets (fix paths)
cp "target/$TARGET/release/$EXE_NAME" target/release/win-portable/
if [ -d "assets" ]; then
    cp -r assets target/release/win-portable/
fi

# create zip archive
cd target/release
powershell -Command "Compress-Archive -Path win-portable\* -DestinationPath flowsurface-x86_64-windows.zip -Force"