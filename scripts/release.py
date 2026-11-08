"""Version checks and fail-closed assembly of GitHub release/update assets."""
import argparse
import datetime
import hashlib
import json
import pathlib
import re
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]


def version():
    rust = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    for path in ["apps/desktop/package.json", "apps/desktop/src-tauri/tauri.conf.json"]:
        assert json.loads((ROOT / path).read_text(encoding="utf-8"))["version"] == rust, f"Version mismatch: {path}"
    assert re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", rust), "Invalid release version"
    return rust


def updater_assets(tag):
    return {
        "windows-x86_64": f"foldo-desktop-{tag}-windows-x64-setup.exe",
        "darwin-aarch64": f"foldo-desktop-{tag}-macos-arm64.app.tar.gz",
        "linux-x86_64": f"foldo-desktop-{tag}-linux-x64.AppImage",
    }


def manifest(directory, tag, repo):
    assert tag == "v" + version(), "Tag must match the application version"
    assets = updater_assets(tag)
    required = list(assets.values()) + [
        f"foldo-desktop-{tag}-macos-arm64.dmg", f"foldo-desktop-{tag}-linux-x64.deb",
        f"foldo-{tag}-windows-x64.zip", f"foldo-{tag}-macos-arm64.tar.gz",
        f"foldo-{tag}-macos-x64.tar.gz", f"foldo-{tag}-linux-x64.tar.gz",
    ]
    platforms = {}
    for platform, asset in assets.items():
        signature = directory / (asset + ".sig")
        assert signature.is_file() and signature.read_text().strip(), f"Missing signature: {signature}"
        platforms[platform] = {
            "signature": signature.read_text().strip(),
            "url": f"https://github.com/{repo}/releases/download/{tag}/{asset}",
        }
    for asset in required:
        assert (directory / asset).is_file() and (directory / asset).stat().st_size > 0, f"Missing asset: {asset}"
    metadata = {
        "version": version(),
        "notes": f"Release notes: https://github.com/{repo}/releases/tag/{tag}",
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    (directory / "latest.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    write_checksums(directory)


def asset_download_urls(metadata, tag, repo, uploaded_assets):
    """Use GitHub's binary asset API, supported by the updater in existing clients."""
    assert metadata["version"] == tag.removeprefix("v"), "Manifest version must match tag"
    assets = updater_assets(tag)
    assert set(metadata["platforms"]) == set(assets), "Incomplete updater platforms"
    urls = {}
    for platform, name in assets.items():
        matches = [asset for asset in uploaded_assets if asset["name"] == name]
        assert len(matches) == 1, f"Missing or duplicate uploaded asset: {name}"
        asset = matches[0]
        assert asset["state"] == "uploaded" and asset["size"] > 0, f"Incomplete uploaded asset: {name}"
        assert type(asset["id"]) is int and asset["id"] > 0, f"Invalid asset ID: {name}"
        expected = f"https://github.com/{repo}/releases/download/{tag}/{name}"
        assert asset["browser_download_url"] == expected, f"Asset belongs to another release: {name}"
        urls[platform] = f"https://api.github.com/repos/{repo}/releases/assets/{asset['id']}"
    for platform, url in urls.items():
        metadata["platforms"][platform]["url"] = url
    return metadata


def link_assets(directory, tag, repo, uploaded_assets):
    path = directory / "latest.json"
    metadata = asset_download_urls(json.loads(path.read_text(encoding="utf-8")), tag, repo, uploaded_assets)
    path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    write_checksums(directory)


def write_checksums(directory):
    checksums = []
    for file in sorted(directory.iterdir()):
        if file.is_file() and file.name != "SHA256SUMS.txt":
            with file.open("rb") as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            checksums.append(f"{digest}  {file.name}")
    (directory / "SHA256SUMS.txt").write_text("\n".join(checksums) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["version", "manifest", "link-assets"])
    parser.add_argument("--directory", type=pathlib.Path, default=ROOT / "dist")
    parser.add_argument("--tag")
    parser.add_argument("--repo", default="Wonvy/Foldo")
    parser.add_argument("--release-assets", type=pathlib.Path)
    args = parser.parse_args()
    if args.command == "version":
        print(version())
    elif args.command == "manifest":
        manifest(args.directory, args.tag, args.repo)
    else:
        if args.release_assets is None:
            parser.error("link-assets requires --release-assets")
        link_assets(args.directory, args.tag, args.repo,
                    json.loads(args.release_assets.read_text(encoding="utf-8")))
