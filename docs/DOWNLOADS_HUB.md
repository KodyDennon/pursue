# Downloads hub integration

PURSUE is published through the private `KodyDennon/downloads-hub` portal repository while retaining ownership of all project-specific public copy and release routing in this repository.

## Public routes

- `https://downloads.kodydennon.com/` — Kody Dennon multi-project download directory.
- `https://downloads.kodydennon.com/PURSUE` — complete PURSUE download, installation, privacy, usage, integrity, and system-selection experience.
- `https://downloads.kodydennon.com/releases/*` — stable public paths that the hub redirects to the direct `releases.kodydennon.com` R2 custom domain.
- `https://downloads.kodydennon.com/latest.json` — reserved updater path; it remains unavailable while updater artifacts and signing are disabled.

## Editing PURSUE content

`downloads/project.json` is the canonical profile consumed by the hub. It owns:

- the PURSUE portal card and detail-page copy;
- platform and GPU selection guidance;
- installer aliases and system requirements;
- capabilities and first-run workflow;
- installation, local-data, network, manual-update, persistence, and trust guidance;
- source, release, and verified-manifest links;
- the independent-project disclaimer.

Validate every edit before committing:

```bash
bun run downloads:validate
```

The profile is public and must never contain credentials. The private hub code treats it as untrusted input, verifies it against an origin allowlist and strict v1 contract, and HTML-escapes all rendered values. The public hub Worker delegates `/PURSUE` to an isolated PURSUE Worker through a Service Binding; that project Worker has no public route. Copy-only changes appear after the five-minute profile cache refresh. Shared layouts, dark PURSUE terminal styling, Kody Dennon hub branding, card-to-detail transitions, structured data, discovery endpoints, D1 counting, and both Worker deployments belong in the private hub repository.

## Download count contract

The hub increments a Cloudflare D1 aggregate only when it receives a `GET` for one of PURSUE's four recognized stable installer aliases. It stores:

- project slug;
- artifact alias;
- UTC date;
- aggregate request count and update timestamp.

It does not store IP addresses, user agents, referrers, cookies, account identifiers, or raw request events. `HEAD` requests, unknown paths, immutable-origin requests, GitHub downloads, interrupted transfers, and direct `releases.kodydennon.com` requests are not counted. The number is therefore labeled “mirror download requests,” not completed downloads or installations.

## Search and answer discovery

The hub provides canonical metadata, Open Graph and Twitter previews, a sitemap, `robots.txt`, `llms.txt`, and visible-copy-backed Schema.org data. The PURSUE route publishes `SoftwareApplication`, `FAQPage`, and `BreadcrumbList` structured data using this profile and the verified release manifest. Keep profile answers direct, factual, and user-readable; do not add invisible keyword variants or claims that are not supported by the product.

## Responsibility boundary

This integration intentionally does not use a git subtree or submodule. Project content is a versioned public data contract; portal implementation has a single private owner. Release building, digest verification, mirroring, two-version retention, and R2 credentials remain in PURSUE. Neither the hub nor project Worker has R2 upload or code-signing credentials. Automatic updater artifacts and signing are currently disabled.

See `downloads/README.md` for the editing contract and `docs/R2_MIRROR_HANDOFF.md` for release-mirror operations.
