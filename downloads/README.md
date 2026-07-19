# PURSUE download profile

`project.json` is the canonical public copy and release-routing profile consumed by the private `KodyDennon/downloads-hub` Worker. Edit this file when PURSUE's download-page wording, system requirements, platform guidance, project links, or release endpoints change.

## Contract

- Keep `schema_version` at `1` unless both repositories are deliberately migrated together.
- Keep `slug` and `route` stable. The public detail page is `https://downloads.kodydennon.com/PURSUE`.
- Every `platforms[].lanes[].alias` and `secondary_alias` must appear once in `release.artifact_aliases` and in the mirrored release manifest.
- `manifest_origin_url` is the direct R2 hostname used by the Worker. The Worker reads it without an edge TTL, validates immutable artifact origins, and resolves stable `downloads.kodydennon.com` installer paths to the current version automatically.
- Copy is treated as untrusted input by the hub and HTML-escaped before rendering.
- Never add credentials, private URLs, analytics keys, or signing material. This file is public even though the hub repository is private.

## Editing and verification

```bash
bun run downloads:validate
```

After a change reaches `main`, the hub refreshes the public profile on its next cache refresh. A portal redeploy is not required for copy-only changes, and a successfully mirrored release appears on the next page or stable-download request. Changes to the schema, route behavior, layouts, animations, or shared styles belong in the private downloads-hub repository and must be coordinated with this contract.

The R2 mirror, two-version retention policy, GitHub configuration, and public integrity checks remain documented in `docs/R2_MIRROR_HANDOFF.md`.
