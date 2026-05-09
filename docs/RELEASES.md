# Releases

Releases are published through GitHub Actions and GitHub Releases.

## Supported Targets

- macOS 26 or newer, Apple Silicon only (`aarch64-apple-darwin`).
- Windows x64 through the default Tauri Windows installer target.

There is no Intel macOS or universal macOS release target.

## Release Automation

The `build-installers` workflow runs on pushes, pull requests, manual dispatch, and tags matching `v*`.

For normal branch pushes and pull requests it runs:

- `bun install --frozen-lockfile`
- `bun run check`
- `bun run build`
- `cargo check`
- `cargo test`
- Tauri installer builds for macOS and Windows

For tags matching `v*`, the workflow publishes a non-draft GitHub Release with downloadable installer assets.

The repository Actions permission is configured with `contents: write` so tag builds can create and update releases.

## Publishing A Release

Manual release path:

```bash
git tag v0.2.1
git push origin v0.2.1
```

Automated version path:

```bash
git commit -m "release: patch"
git push origin main
```

The `Auto Version and Release` workflow accepts:

- `release: patch`
- `release: minor`
- `release: major`
- `release: 1.2.3`

It updates `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`, commits the version bump when needed, and pushes the matching `v*` tag.

## Signing

The public release workflow intentionally publishes unsigned installers by default. Invalid or incomplete signing secrets must not block downloadable releases.

Unsigned builds can still be downloaded from GitHub Releases, but macOS and Windows may show trust warnings during install. Add a separate signed release lane only after Apple Developer ID, notarization, and Tauri signing secrets have been verified in CI.
