#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$repo_root/target"}
dist_dir="$repo_root/dist"

os=$(uname -s)
arch=$(uname -m)

case "$os" in
  Darwin)
    src="$target_dir/release/liblitelease_ext.dylib"
    ext="dylib"
    platform="macos"
    ;;
  Linux)
    src="$target_dir/release/liblitelease_ext.so"
    ext="so"
    platform="linux"
    ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT)
    src="$target_dir/release/litelease_ext.dll"
    ext="dll"
    platform="windows"
    ;;
  *)
    echo "unsupported platform: $os" >&2
    exit 1
    ;;
esac

case "$arch" in
  arm64|aarch64) arch_tag="arm64" ;;
  x86_64|amd64) arch_tag="x86_64" ;;
  *)
    echo "unsupported architecture: $arch" >&2
    exit 1
    ;;
esac

asset_name="litelease-extension-${platform}-${arch_tag}.${ext}"

if [ ! -f "$src" ]; then
  echo "missing built extension artifact at $src" >&2
  echo "run: cargo build -p litelease-extension --release" >&2
  exit 1
fi

mkdir -p "$dist_dir"
cp "$src" "$dist_dir/$asset_name"
"$repo_root/scripts/write_sha256.sh" \
  "$dist_dir/$asset_name" \
  "$dist_dir/$asset_name.sha256"

echo "staged:"
echo "  $dist_dir/$asset_name"
echo "  $dist_dir/$asset_name.sha256"
