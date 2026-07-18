#!/usr/bin/env bash
set -euo pipefail

version="7869"
case "$(uname -s)" in
  Darwin)
    archive="pdfium-mac-arm64.tgz"
    expected="935a50329d5f72466b2058f92f2c4a8f9e541abc8f3149b1994d078dec4190e1"
    library_source="lib/libpdfium.dylib"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    archive="pdfium-win-x64.tgz"
    expected="d1a2b39c300f62daeec94f3a648a31d83d18605707bfdc5504d818d42cab13ce"
    library_source="bin/pdfium.dll"
    ;;
  *)
    echo "Unsupported installer platform: $(uname -s)" >&2
    exit 1
    ;;
esac

stage_dir="src-tauri/assets/pdfium"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

curl --fail --location --retry 5 --retry-all-errors \
  "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${version}/${archive}" \
  --output "${work_dir}/${archive}"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "${work_dir}/${archive}" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "${work_dir}/${archive}" | awk '{print $1}')"
fi
if [[ "$actual" != "$expected" ]]; then
  echo "PDFium checksum mismatch: expected ${expected}, got ${actual}" >&2
  exit 1
fi

tar -xzf "${work_dir}/${archive}" -C "$work_dir"
mkdir -p "$stage_dir/licenses"
cp "${work_dir}/${library_source}" "$stage_dir/"
cp "${work_dir}/LICENSE" "${work_dir}/VERSION" "$stage_dir/"
cp "${work_dir}/licenses/"* "$stage_dir/licenses/"
test -s "$stage_dir/$(basename "$library_source")"
echo "Staged PDFium Chromium ${version} (${expected})"
