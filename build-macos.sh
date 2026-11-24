#!/usr/bin/env bash
set -euo pipefail

echo "=== Grain Video & Audio Editor — 100% Pure Rust macOS Build ==="

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: Rust is not installed. Install via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi

if ! command -v cargo-bundle >/dev/null 2>&1; then
  echo "Installing cargo-bundle for macOS .app packaging..."
  cargo install cargo-bundle
fi

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "Warning: ffmpeg not found. Install via Homebrew for hardware export: brew install ffmpeg"
fi

echo "Building Universal macOS .app bundle (Apple Silicon + Intel)..."
rustup target add aarch64-apple-darwin x86_64-apple-darwin || true
cargo bundle --release --target universal-apple-darwin || cargo bundle --release

APP_PATH=$(find target -name "Grain Video Editor.app" | head -n 1)
echo "Built macOS App: ${APP_PATH}"

if [ -n "${APP_PATH}" ] && command -v hdiutil >/dev/null 2>&1; then
  DMG_PATH="target/Grain-Video-Editor-macOS.dmg"
  rm -f "${DMG_PATH}"
  hdiutil create -volname "Grain Video Editor" -srcfolder "${APP_PATH}" -ov -format UDZO "${DMG_PATH}"
  echo "Created macOS Installer DMG: ${DMG_PATH}"
fi
