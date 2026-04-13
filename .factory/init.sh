#!/bin/bash
set -e

cd /Users/js/code/rust/lab

# Initialize cargo project if not already done
if [ ! -f Cargo.toml ]; then
  echo "Cargo project not initialized yet - first feature will set this up"
fi

# Build if Cargo.toml exists
if [ -f Cargo.toml ]; then
  cargo build 2>/dev/null || true
fi
