#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <input-file> <output-file>" >&2
  exit 1
fi

input=$1
output=$2

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$input" | awk '{print $1 "  " FILENAME}' FILENAME="$(basename "$input")" >"$output"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$input" | awk '{print $1 "  " FILENAME}' FILENAME="$(basename "$input")" >"$output"
elif command -v openssl >/dev/null 2>&1; then
  hash=$(openssl dgst -sha256 -r "$input" | awk '{print $1}')
  printf '%s  %s\n' "$hash" "$(basename "$input")" >"$output"
else
  echo "no sha256 tool found (need sha256sum, shasum, or openssl)" >&2
  exit 1
fi
