# Cloudflare R2 Release Mirror — CLI/API Handoff

## Mission and boundaries

Complete the R2 production mirror from the trusted Mac that already has Cloudflare, GitHub, and DNS credentials. Use authenticated CLIs and HTTPS APIs only—no dashboard instructions. Never echo, paste, commit, or log credentials. Pipe secret values directly into `gh secret set`.

Repository implementation already present:

- `.github/scripts/mirror-release-to-r2.sh`
- `.github/workflows/mirror-release.yml`
- `mirror-r2` in `.github/workflows/release.yml`
- signed three-lane updater generation in `scripts/generate-updater-manifest.mjs`

The mirror uploads immutable release objects first, verifies GitHub digests and R2 metadata, then advances stable aliases. GitHub remains the release source of truth and fallback.

## State at Windows handoff

- Repository: `KodyDennon/pursue`
- Last pre-handoff release: `v0.10.1`
- Expected first updater-enabled release: `v0.10.2` (confirm with `gh release list`)
- R2 GitHub variables were not configured on the Windows machine.
- The app currently has the GitHub updater endpoint only. Add the real R2 custom-domain endpoint only after public verification.
- `v0.10.1` has five legacy manual installers but no seven-file signed updater set. It can be mirrored for manual downloads only with a release-specific override; do not expect an updater manifest from that backfill.

## 1. Audit authenticated CLIs

```bash
set -euo pipefail
gh auth status
npx wrangler whoami --json | jq '{accounts, authType: (.authType // .auth_type // "unknown")}'
aws --version
git -C /path/to/pursue status --short
```

Select, do not guess, the Cloudflare account and an existing zone in that same account:

```bash
export R2_ACCOUNT_ID='32-character-id-from-wrangler'
export R2_BUCKET_NAME='pursue-releases'
export R2_PUBLIC_HOST='downloads.example.com'
export R2_PUBLIC_BASE_URL="https://$R2_PUBLIC_HOST"
```

## 2. Create or reuse the bucket

```bash
npx wrangler r2 bucket list
npx wrangler r2 bucket create "$R2_BUCKET_NAME" # only if absent
npx wrangler r2 bucket list
```

Use Standard storage. Do not enable or CNAME the rate-limited `r2.dev` development origin.

## 3. Attach the custom domain through Cloudflare API

Obtain Wrangler's authenticated token response without printing it:

```bash
cf_auth="$(npx wrangler auth token --json)"
cf_auth_type="$(jq -r .type <<<"$cf_auth")"
```

Define an API helper using the returned auth type:

```bash
cf_api() {
  local method="$1" path="$2" body="${3:-}"
  local -a headers=(--header 'Content-Type: application/json')
  case "$cf_auth_type" in
    api_token|oauth)
      headers+=(--header "Authorization: Bearer $(jq -r .token <<<"$cf_auth")")
      ;;
    api_key)
      headers+=(
        --header "X-Auth-Key: $(jq -r .key <<<"$cf_auth")"
        --header "X-Auth-Email: $(jq -r .email <<<"$cf_auth")"
      )
      ;;
    *) printf 'Unsupported Wrangler auth type: %s\n' "$cf_auth_type" >&2; return 1 ;;
  esac
  if [[ -n "$body" ]]; then
    curl --fail-with-body --silent --show-error --request "$method" \
      "${headers[@]}" --data "$body" "https://api.cloudflare.com/client/v4$path"
  else
    curl --fail-with-body --silent --show-error --request "$method" \
      "${headers[@]}" "https://api.cloudflare.com/client/v4$path"
  fi
}
```

Resolve the correct zone by hostname suffix and retain its exact ID:

```bash
zones="$(cf_api GET "/zones?account.id=$R2_ACCOUNT_ID&per_page=50")"
jq -e '.success == true' <<<"$zones" >/dev/null
jq -r '.result[] | [.id, .name] | @tsv' <<<"$zones"
export R2_ZONE_ID='verified-zone-id'
```

List existing custom domains and attach only if absent:

```bash
cf_api GET "/accounts/$R2_ACCOUNT_ID/r2/buckets/$R2_BUCKET_NAME/domains/custom" | jq .
body="$(jq -nc --arg domain "$R2_PUBLIC_HOST" --arg zoneId "$R2_ZONE_ID" \
  '{domain: $domain, zoneId: $zoneId, enabled: true}')"
cf_api POST "/accounts/$R2_ACCOUNT_ID/r2/buckets/$R2_BUCKET_NAME/domains/custom" "$body" \
  | jq -e '.success == true'
```

Poll both ownership and SSL status until active:

```bash
for attempt in $(seq 1 60); do
  status="$(cf_api GET "/accounts/$R2_ACCOUNT_ID/r2/buckets/$R2_BUCKET_NAME/domains/custom/$R2_PUBLIC_HOST")"
  if jq -e '.success == true and .result.enabled == true and .result.status.ownership == "active" and .result.status.ssl == "active"' <<<"$status" >/dev/null; then
    break
  fi
  sleep 10
done
curl --fail --silent --show-error --head "$R2_PUBLIC_BASE_URL/" || true
```

## 4. Configure GitHub without exposing credentials

Use an existing bucket-scoped R2 Object Read & Write S3 credential from the Mac. If stored in an AWS profile:

```bash
export R2_AWS_PROFILE='cloudflare-r2'
aws configure export-credentials --profile "$R2_AWS_PROFILE" | jq -r .AccessKeyId \
  | gh secret set R2_ACCESS_KEY_ID --repo KodyDennon/pursue
aws configure export-credentials --profile "$R2_AWS_PROFILE" | jq -r .SecretAccessKey \
  | gh secret set R2_SECRET_ACCESS_KEY --repo KodyDennon/pursue
```

If already exported:

```bash
test -n "${AWS_ACCESS_KEY_ID:-}" && test -n "${AWS_SECRET_ACCESS_KEY:-}"
printf %s "$AWS_ACCESS_KEY_ID" | gh secret set R2_ACCESS_KEY_ID --repo KodyDennon/pursue
printf %s "$AWS_SECRET_ACCESS_KEY" | gh secret set R2_SECRET_ACCESS_KEY --repo KodyDennon/pursue
```

If no long-lived R2 S3 credential exists, create a bucket-scoped R2 API token through Cloudflare's authenticated Create Token API, then derive the S3 fields according to Cloudflare's R2 token contract (token ID is the access key ID; SHA-256 of the returned token value is the secret). Do this entirely in-memory and immediately pipe the fields to GitHub. The release workflow currently expects long-lived credentials and does not accept a temporary session token.

Set non-secret variables only after validating their values:

```bash
gh variable set R2_ACCOUNT_ID --repo KodyDennon/pursue --body "$R2_ACCOUNT_ID"
gh variable set R2_BUCKET_NAME --repo KodyDennon/pursue --body "$R2_BUCKET_NAME"
gh variable set R2_PUBLIC_BASE_URL --repo KodyDennon/pursue --body "$R2_PUBLIC_BASE_URL"
```

Do not store Tauri signing keys, Cloudflare tokens, or S3 secrets as GitHub variables.

## 5. Backfill and verify

Backfill `v0.10.1` for manual installer aliases if desired:

```bash
gh workflow run mirror-release.yml --repo KodyDennon/pursue -f tag=v0.10.1
```

Then mirror the newest updater-enabled tag (replace after checking the actual release list):

```bash
export RELEASE_TAG="$(gh release list --repo KodyDennon/pursue --limit 1 --json tagName --jq '.[0].tagName')"
gh workflow run mirror-release.yml --repo KodyDennon/pursue -f tag="$RELEASE_TAG"
run_id="$(gh run list --repo KodyDennon/pursue --workflow mirror-release.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run watch "$run_id" --repo KodyDennon/pursue --exit-status
```

For the updater-enabled tag, the workflow requires exactly four installers plus these seven updater assets: three bundles, three `.sig` files, and `latest.json`. It publishes:

- immutable bytes under `releases/$RELEASE_TAG/`;
- four stable installer aliases under `releases/latest/`;
- `release-manifest.json` and `releases/latest/manifest.json` for manual downloads;
- `releases/$RELEASE_TAG/updater.json`, `releases/latest/updater.json`, and top-level `latest.json` for signed updates.

Verify public metadata and a complete large download:

```bash
curl --fail --location --silent --show-error "$R2_PUBLIC_BASE_URL/releases/latest/manifest.json" | jq .
curl --fail --location --silent --show-error "$R2_PUBLIC_BASE_URL/latest.json" | jq .
curl --fail --location --remote-name "$R2_PUBLIC_BASE_URL/releases/latest/windows-cuda.msi"
expected="$(curl --fail --silent "$R2_PUBLIC_BASE_URL/releases/latest/manifest.json" \
  | jq -r '.artifacts[] | select(.alias == "windows-cuda.msi") | .sha256')"
printf '%s  %s\n' "$expected" windows-cuda.msi | shasum -a 256 --check
```

## 6. Enable automatic mirroring and wire the updater endpoint

Only after the public checks pass:

```bash
gh variable set R2_MIRROR_ENABLED --repo KodyDennon/pursue --body true
```

Add the verified R2 updater endpoint before GitHub in `src-tauri/tauri.conf.json`, retaining GitHub as fallback:

```json
"endpoints": [
  "https://downloads.example.com/latest.json",
  "https://github.com/KodyDennon/pursue/releases/latest/download/latest.json"
]
```

Run the full validation suite, bump the patch version, and cut another release so installed clients actually receive the R2-first endpoint. Never commit a guessed hostname.

## Completion checklist

- [ ] GitHub, Wrangler, AWS, and DNS/account identities verified by CLI.
- [ ] Bucket exists and the custom domain reports active ownership and SSL.
- [ ] Bucket-scoped S3 credentials are encrypted GitHub secrets.
- [ ] `R2_ACCOUNT_ID`, `R2_BUCKET_NAME`, and `R2_PUBLIC_BASE_URL` are GitHub variables.
- [ ] `v0.10.1` manual installer backfill passes if requested.
- [ ] New updater-enabled tag mirrors four installers and seven updater assets.
- [ ] Public manual and updater manifests parse and point at immutable HTTPS objects.
- [ ] Full CUDA installer download matches the manifest SHA-256.
- [ ] `R2_MIRROR_ENABLED=true` only after verification.
- [ ] Verified R2 endpoint is committed before GitHub fallback and released in a subsequent patch.
