#!/usr/bin/env bash
set -euo pipefail

echo "=== Grain Video Editor — macOS Build Script ==="

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: Rust (cargo) is not installed. Install via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "Error: Node.js / npm is not installed."
  exit 1
fi

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "Warning: ffmpeg not found on PATH. Install via Homebrew for native hardware export: brew install ffmpeg pkg-config"
fi

echo "1. Installing npm dependencies..."
npm install

echo "2. Adding macOS Rust targets (Apple Silicon + Intel)..."
rustup target add aarch64-apple-darwin x86_64-apple-darwin || true

echo "3. Building Universal macOS .app and .dmg..."
npm run build -- --target universal-apple-darwin

echo ""
echo "Build complete! Artifacts:"
echo "  App: src-tauri/target/universal-apple-darwin/release/bundle/macos/Grain Video Editor.app"
echo "  DMG: src-tauri/target/universal-apple-darwin/release/bundle/dmg/Grain Video Editor_0.1.0_universal.dmg"
