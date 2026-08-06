#!/bin/bash
# Vendors static ffmpeg/ffprobe binaries into src-tauri/binaries/ as Tauri
# sidecars (see tauri.conf.json bundle.externalBin). Not committed to git -
# run this once locally and once in CI before every build.
#
# Usage: scripts/fetch-ffmpeg.sh [linux|windows|all]
set -euo pipefail

TARGET="${1:-linux}"
# BtbN publishes under a single rolling "latest" tag (verified against the
# live releases API) rather than dated tags, so true reproducibility would
# require pinning by asset checksum. Good enough for now; re-run this script
# to pick up newer ffmpeg builds.
BASE_URL="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${SCRIPT_DIR}/../src-tauri/binaries"
mkdir -p "${BIN_DIR}"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

fetch_linux() {
  echo "Fetching static ffmpeg/ffprobe for x86_64-unknown-linux-gnu..."
  curl -fL "${BASE_URL}/ffmpeg-master-latest-linux64-gpl.tar.xz" \
    -o "${WORK_DIR}/ffmpeg-linux.tar.xz"
  tar -xf "${WORK_DIR}/ffmpeg-linux.tar.xz" -C "${WORK_DIR}"
  local extracted
  extracted="$(find "${WORK_DIR}" -maxdepth 1 -type d -name 'ffmpeg-*-linux64-gpl')"
  cp "${extracted}/bin/ffmpeg" "${BIN_DIR}/ffmpeg-x86_64-unknown-linux-gnu"
  cp "${extracted}/bin/ffprobe" "${BIN_DIR}/ffprobe-x86_64-unknown-linux-gnu"
  chmod +x "${BIN_DIR}/ffmpeg-x86_64-unknown-linux-gnu" "${BIN_DIR}/ffprobe-x86_64-unknown-linux-gnu"
}

fetch_windows() {
  echo "Fetching static ffmpeg/ffprobe for x86_64-pc-windows-gnu..."
  curl -fL "${BASE_URL}/ffmpeg-master-latest-win64-gpl.zip" \
    -o "${WORK_DIR}/ffmpeg-windows.zip"
  unzip -q "${WORK_DIR}/ffmpeg-windows.zip" -d "${WORK_DIR}"
  local extracted
  extracted="$(find "${WORK_DIR}" -maxdepth 1 -type d -name 'ffmpeg-*-win64-gpl')"
  cp "${extracted}/bin/ffmpeg.exe" "${BIN_DIR}/ffmpeg-x86_64-pc-windows-gnu.exe"
  cp "${extracted}/bin/ffprobe.exe" "${BIN_DIR}/ffprobe-x86_64-pc-windows-gnu.exe"
}

case "${TARGET}" in
  linux) fetch_linux ;;
  windows) fetch_windows ;;
  all) fetch_linux; fetch_windows ;;
  *) echo "Usage: $0 [linux|windows|all]" >&2; exit 1 ;;
esac

echo "Done. Vendored binaries in ${BIN_DIR}:"
ls -lh "${BIN_DIR}"
