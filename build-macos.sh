#!/usr/bin/env bash
set -euo pipefail

echo "=== Grain Video & Audio Studio — 100% Pure Rust macOS M3 (aarch64-apple-darwin) Build ==="

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: Rust is not installed. Install via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi

if ! command -v cargo-bundle >/dev/null 2>&1; then
  echo "Installing cargo-bundle for macOS .app packaging..."
  cargo install cargo-bundle
fi

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "Warning: ffmpeg not found. Install via Homebrew for hardware VideoToolbox export: brew install ffmpeg"
fi

echo "Building native Apple Silicon (aarch64-apple-darwin) .app bundle..."
rustup target add aarch64-apple-darwin || true
RUSTFLAGS="-C target-cpu=apple-m3" cargo bundle --release --target aarch64-apple-darwin

APP_PATH="target/aarch64-apple-darwin/release/bundle/osx/Grain Video Editor.app"
echo "Built macOS App: ${APP_PATH}"

if [ -d "${APP_PATH}" ] && command -v hdiutil >/dev/null 2>&1; then
  DMG_PATH="target/Grain-Video-Editor-macOS-M3.dmg"
  rm -f "${DMG_PATH}"
  hdiutil create -volname "Grain Video Editor" -srcfolder "${APP_PATH}" -ov -format UDZO "${DMG_PATH}"
  echo "Created macOS Installer DMG: ${DMG_PATH}"
fi
