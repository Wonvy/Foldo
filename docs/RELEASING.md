# Releasing Foldo

[English](../README.md) · [简体中文](../README.zh-CN.md)

## Automatic publishing

The Build workflow tests all three platforms, builds the frontend, packages the CLI and desktop app, then publishes a **complete** GitHub Release. Installers are ordinary release assets; users do not need an Actions login.

1. Update the version in `Cargo.toml`, the `foldo-core` dependency in `crates/foldo-cli/Cargo.toml`, `apps/desktop/package.json`, `apps/desktop/package-lock.json`, and `apps/desktop/src-tauri/tauri.conf.json`. Run `cargo check` to refresh `Cargo.lock`.
2. Commit and push to `main`. A version without a published Release is automatically published as `v<version>` once all checks and packages succeed.
3. Later commits with the same version still build Actions artifacts, but **do not overwrite** the published version. Bump the version for the next update.

You can also push a matching `v*` tag or run Build manually. Tag and application versions must match. A suffix such as `-beta.1` produces a prerelease, which is excluded from the stable updater feed.

Publishing uses a draft while assets are uploaded. Missing installers or update signatures fail the job; an incomplete release is never deliberately published. Re-running a failed workflow can finish a draft. Published releases are immutable by this workflow.

## Update signing

The repository Actions secret `TAURI_SIGNING_PRIVATE_KEY` contains the updater signing key. Its matching public key is in `tauri.conf.json`. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is only needed for a password-protected key.

Keep a backup of the private key outside Git. On the initial release workstation it is stored at `~/.tauri/foldo-updater.key`. Never commit it, paste it into a workflow, or print it in logs. Do not replace the key for normal releases: installed apps trust the original public key.

The workflow creates signed Windows NSIS, macOS application archive, and Linux AppImage updates. `scripts/release.py` requires every expected artifact and signature, writes `latest.json`, and computes `SHA256SUMS.txt`. The updater uses:

```
https://github.com/Wonvy/Foldo/releases/latest/download/latest.json
```

Updater signing authenticates updates; it is separate from Windows Authenticode signing or Apple notarization. Those are not configured here. Linux `.deb` users install new packages manually; AppImage users can update in the app.

## Validation

```sh
python scripts/release.py version
python -m unittest discover -s scripts -p 'test_*.py'
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cd apps/desktop
npm ci
npm run build
```

After publishing, verify the installer links in the README, the update manifest's version and platform URLs, and all referenced assets. Test installing an update from an older release when a second signed version is available.
