#!/usr/bin/env bash
# Downloads the sherpa-onnx Windows x64 shared prebuilt and extracts it to
# frontend/src-tauri/vendor/sherpa-onnx/. Required for the diarization-onnx
# Cargo feature on Windows — sherpa-onnx-sys's build.rs expects the libs to
# exist at the path referenced by SHERPA_ONNX_LIB_DIR (set in .cargo/config.toml).
#
# Re-run after a sherpa-onnx version bump or cargo clean that wiped vendor/.
# Idempotent — skips if the expected sherpa-onnx-c-api.dll is already present.

set -euo pipefail

VERSION="${SHERPA_ONNX_VERSION:-v1.12.40}"
VARIANT="${SHERPA_ONNX_VARIANT:-win-x64-shared-MD-Release-no-tts}"
ARCHIVE_NAME="sherpa-onnx-${VERSION}-${VARIANT}.tar.bz2"
URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/${VERSION}/${ARCHIVE_NAME}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENDOR_DIR="${SCRIPT_DIR}/../vendor"
TARGET_DIR="${VENDOR_DIR}/sherpa-onnx"

if [[ -f "${TARGET_DIR}/lib/sherpa-onnx-c-api.dll" ]]; then
  echo "sherpa-onnx already present at ${TARGET_DIR}. Delete the dir to force re-download."
  exit 0
fi

mkdir -p "${VENDOR_DIR}"
cd "${VENDOR_DIR}"

echo "Downloading ${URL}"
curl -sSL -o "${ARCHIVE_NAME}" "${URL}"
echo "Extracting"
tar -xjf "${ARCHIVE_NAME}"
rm "${ARCHIVE_NAME}"

EXTRACTED="sherpa-onnx-${VERSION}-${VARIANT}"
rm -rf "${TARGET_DIR}"
mv "${EXTRACTED}" "${TARGET_DIR}"
echo "Installed ${TARGET_DIR}"
ls "${TARGET_DIR}/lib" | head -20
