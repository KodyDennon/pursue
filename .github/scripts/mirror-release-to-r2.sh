#!/usr/bin/env bash
set -Eeuo pipefail

die() {
  echo "R2 mirror error: $*" >&2
  exit 1
}

for command in aws curl gh jq sha256sum; do
  command -v "$command" >/dev/null 2>&1 || die "required command '$command' is unavailable"
done

tag="${1:-}"
if [[ -z "$tag" ]]; then
  tag="$(gh release view --json tagName --jq .tagName)"
fi

[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]] ||
  die "release tag '$tag' is not a supported semantic version"

required_environment=(
  R2_ACCOUNT_ID
  R2_ACCESS_KEY_ID
  R2_SECRET_ACCESS_KEY
  R2_BUCKET_NAME
  R2_PUBLIC_BASE_URL
)
for variable in "${required_environment[@]}"; do
  [[ -n "${!variable:-}" ]] || die "$variable is not configured"
done

[[ "$R2_ACCOUNT_ID" =~ ^[0-9a-fA-F]{32}$ ]] || die "R2_ACCOUNT_ID must be a 32-character Cloudflare account ID"
[[ "$R2_BUCKET_NAME" =~ ^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$ ]] ||
  die "R2_BUCKET_NAME must be 3-63 lowercase letters, numbers, or hyphens and cannot start or end with a hyphen"
[[ "$R2_PUBLIC_BASE_URL" == https://* ]] || die "R2_PUBLIC_BASE_URL must use HTTPS"

public_base="${R2_PUBLIC_BASE_URL%/}"
endpoint="https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com"
repository="${GITHUB_REPOSITORY:-KodyDennon/pursue}"
export AWS_ACCESS_KEY_ID="$R2_ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$R2_SECRET_ACCESS_KEY"
export AWS_DEFAULT_REGION=auto
export AWS_EC2_METADATA_DISABLED=true
export AWS_RETRY_MODE=adaptive
export AWS_MAX_ATTEMPTS=12

working_directory="$(mktemp -d)"
trap 'rm -rf -- "$working_directory"' EXIT
asset_directory="$working_directory/assets"
mkdir -p "$asset_directory"

release_json="$working_directory/release.json"
gh release view "$tag" --repo "$repository" \
  --json tagName,url,isDraft,isPrerelease,publishedAt,assets > "$release_json"

[[ "$(jq -r .tagName "$release_json")" == "$tag" ]] || die "GitHub returned a different release tag"
[[ "$(jq -r .isDraft "$release_json")" == false ]] || die "refusing to mirror draft release $tag"

mapfile -t installer_names < <(
  jq -r '.assets[].name' "$release_json" |
    grep -E '(_aarch64\.dmg|_x64-cuda_en-US\.msi|_x64-setup\.exe|_x64_en-US\.msi)$'
)
[[ "${#installer_names[@]}" -eq 4 ]] ||
  die "expected exactly four production installers for $tag; found ${#installer_names[@]}"

mapfile -t updater_names < <(
  jq -r '.assets[].name' "$release_json" |
    grep -E '(\.app\.tar\.gz(\.sig)?$|\.nsis\.zip(\.sig)?$|\.msi\.zip(\.sig)?$|^latest\.json$)' || true
)
if [[ "${REQUIRE_UPDATER_ASSETS:-false}" == "true" && "${#updater_names[@]}" -ne 7 ]]; then
  die "expected seven signed updater assets for $tag; found ${#updater_names[@]}"
fi
if [[ "${#updater_names[@]}" -ne 0 && "${#updater_names[@]}" -ne 7 ]]; then
  die "refusing a partial updater mirror for $tag; found ${#updater_names[@]} of seven assets"
fi

asset_names=("${installer_names[@]}" "${updater_names[@]}")

declare -A aliases
for name in "${installer_names[@]}"; do
  case "$name" in
    *_aarch64.dmg) aliases["$name"]="macos-apple-silicon.dmg" ;;
    *_x64-cuda_en-US.msi) aliases["$name"]="windows-cuda.msi" ;;
    *_x64-setup.exe) aliases["$name"]="windows-directml-setup.exe" ;;
    *_x64_en-US.msi) aliases["$name"]="windows-directml.msi" ;;
    *) die "no stable alias is defined for $name" ;;
  esac
done
unique_alias_count="$(printf '%s\n' "${aliases[@]}" | sort -u | wc -l | tr -d ' ')"
[[ "$unique_alias_count" -eq 4 ]] || die "release assets did not map to four unique platform aliases"

echo "Downloading ${#asset_names[@]} GitHub release assets for $tag"
for name in "${asset_names[@]}"; do
  gh release download "$tag" --repo "$repository" --pattern "$name" --dir "$asset_directory"
done

artifacts='[]'
for name in "${asset_names[@]}"; do
  file="$asset_directory/$name"
  [[ -f "$file" ]] || die "downloaded asset is missing: $name"

  expected_size="$(jq -r --arg name "$name" '.assets[] | select(.name == $name) | .size' "$release_json")"
  expected_digest="$(jq -r --arg name "$name" '.assets[] | select(.name == $name) | .digest' "$release_json")"
  actual_size="$(stat -c %s "$file")"
  actual_sha256="$(sha256sum "$file" | cut -d ' ' -f 1)"

  [[ "$actual_size" == "$expected_size" ]] || die "$name size mismatch after GitHub download"
  [[ "$expected_digest" == "sha256:$actual_sha256" ]] || die "$name SHA-256 mismatch after GitHub download"

  case "$name" in
    *.dmg) content_type="application/x-apple-diskimage" ;;
    *.exe) content_type="application/vnd.microsoft.portable-executable" ;;
    *.msi) content_type="application/x-msi" ;;
    *) content_type="application/octet-stream" ;;
  esac

  versioned_key="releases/$tag/$name"
  echo "Uploading $name to R2 ($actual_size bytes)"
  aws s3 cp "$file" "s3://$R2_BUCKET_NAME/$versioned_key" \
    --endpoint-url "$endpoint" \
    --only-show-errors \
    --no-progress \
    --content-type "$content_type" \
    --cache-control 'public, max-age=31536000, immutable' \
    --metadata "sha256=$actual_sha256,release=$tag,source=github-release"

  remote_head="$(aws s3api head-object \
    --endpoint-url "$endpoint" \
    --bucket "$R2_BUCKET_NAME" \
    --key "$versioned_key")"
  [[ "$(jq -r .ContentLength <<< "$remote_head")" == "$actual_size" ]] || die "$name R2 size verification failed"
  [[ "$(jq -r '.Metadata.sha256 // empty' <<< "$remote_head")" == "$actual_sha256" ]] || die "$name R2 SHA-256 metadata verification failed"

  if [[ -n "${aliases[$name]:-}" ]]; then
    alias_name="${aliases[$name]}"
    github_url="$(jq -r --arg name "$name" '.assets[] | select(.name == $name) | .url' "$release_json")"
    artifacts="$(jq -c \
      --arg name "$name" \
      --arg alias "$alias_name" \
      --arg sha256 "$actual_sha256" \
      --argjson bytes "$actual_size" \
      --arg mirror_url "$public_base/$versioned_key" \
      --arg stable_url "$public_base/releases/latest/$alias_name" \
      --arg github_url "$github_url" \
      '. + [{name: $name, alias: $alias, bytes: $bytes, sha256: $sha256, mirror_url: $mirror_url, stable_url: $stable_url, github_fallback_url: $github_url}]' \
      <<< "$artifacts")"
  fi
done

manifest="$working_directory/manifest.json"
jq -n \
  --arg tag "$tag" \
  --arg version "${tag#v}" \
  --arg published_at "$(jq -r .publishedAt "$release_json")" \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg release_page "$(jq -r .url "$release_json")" \
  --arg public_base_url "$public_base" \
  --argjson artifacts "$artifacts" \
  '{schema_version: 1, tag: $tag, version: $version, published_at: $published_at, generated_at: $generated_at, release_page: $release_page, public_base_url: $public_base_url, artifacts: $artifacts}' \
  > "$manifest"

manifest_sha256="$(sha256sum "$manifest" | cut -d ' ' -f 1)"
aws s3 cp "$manifest" "s3://$R2_BUCKET_NAME/releases/$tag/manifest.json" \
  --endpoint-url "$endpoint" --only-show-errors --no-progress \
  --content-type 'application/json; charset=utf-8' \
  --cache-control 'public, max-age=31536000, immutable' \
  --metadata "sha256=$manifest_sha256,release=$tag,source=github-release"

# Publish stable aliases only after every immutable object and its manifest has passed
# integrity checks. This prevents clients from observing a partially mirrored release.
for name in "${installer_names[@]}"; do
  alias_name="${aliases[$name]}"
  sha256="$(jq -r --arg name "$name" '.[] | select(.name == $name) | .sha256' <<< "$artifacts")"
  case "$name" in
    *.dmg) content_type="application/x-apple-diskimage" ;;
    *.exe) content_type="application/vnd.microsoft.portable-executable" ;;
    *.msi) content_type="application/x-msi" ;;
  esac
  aws s3api copy-object \
    --endpoint-url "$endpoint" \
    --bucket "$R2_BUCKET_NAME" \
    --copy-source "$R2_BUCKET_NAME/releases/$tag/$name" \
    --key "releases/latest/$alias_name" \
    --metadata-directive REPLACE \
    --metadata "sha256=$sha256,release=$tag,source=github-release" \
    --content-type "$content_type" \
    --cache-control 'public, max-age=300, must-revalidate' >/dev/null
done

for key in releases/latest/manifest.json release-manifest.json; do
  aws s3 cp "$manifest" "s3://$R2_BUCKET_NAME/$key" \
    --endpoint-url "$endpoint" --only-show-errors --no-progress \
    --content-type 'application/json; charset=utf-8' \
    --cache-control 'public, max-age=300, must-revalidate' \
    --metadata "sha256=$manifest_sha256,release=$tag,source=github-release"
done

if [[ "${#updater_names[@]}" -eq 7 ]]; then
  github_updater="$asset_directory/latest.json"
  r2_updater="$working_directory/updater.json"
  jq -e '
    (.platforms | keys | sort) == (["macos-metal-aarch64", "windows-cuda-x86_64", "windows-directml-x86_64"] | sort)
  ' "$github_updater" >/dev/null || die "GitHub updater manifest has incorrect release lanes"

  jq --arg base "$public_base/releases/$tag" '
    .platforms |= with_entries(
      .value.url = ($base + "/" + (.value.url | split("/") | last))
    )
  ' "$github_updater" > "$r2_updater"

  for target in macos-metal-aarch64 windows-cuda-x86_64 windows-directml-x86_64; do
    bundle_name="$(jq -r --arg target "$target" '.platforms[$target].url | split("/") | last' "$r2_updater")"
    signature="$(jq -r --arg target "$target" '.platforms[$target].signature' "$r2_updater")"
    [[ -f "$asset_directory/$bundle_name" ]] || die "updater bundle missing for $target"
    [[ "$(tr -d '\r\n' < "$asset_directory/$bundle_name.sig")" == "$signature" ]] ||
      die "updater signature mismatch for $target"
  done

  updater_sha256="$(sha256sum "$r2_updater" | cut -d ' ' -f 1)"
  for key in "releases/$tag/updater.json" releases/latest/updater.json latest.json; do
    aws s3 cp "$r2_updater" "s3://$R2_BUCKET_NAME/$key" \
      --endpoint-url "$endpoint" --only-show-errors --no-progress \
      --content-type 'application/json; charset=utf-8' \
      --cache-control 'public, max-age=300, must-revalidate' \
      --metadata "sha256=$updater_sha256,release=$tag,source=github-release"
  done
fi

echo "Verifying public mirror URLs"
curl --fail --silent --show-error --location --retry 5 --retry-all-errors \
  "$public_base/releases/latest/manifest.json" -o "$working_directory/public-manifest.json"
jq -e --arg tag "$tag" '.tag == $tag and (.artifacts | length == 5)' \
  "$working_directory/public-manifest.json" >/dev/null || die "public manifest verification failed"

for name in "${asset_names[@]}"; do
  versioned_key="releases/$tag/$name"
  curl --fail --silent --show-error --location --retry 5 --retry-all-errors --head \
    "$public_base/$versioned_key" >/dev/null || die "public object is unavailable: $name"
done

echo "R2 mirror complete: $public_base/releases/latest/manifest.json"
