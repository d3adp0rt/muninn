# Release

Releases are built by `.github/workflows/release.yml` on a `v*` tag.

## Signing key

The release keystore is **not** in the repository and must never be committed.
The maintainer keeps it outside the working tree and backs it up; losing it
means no future build can update an installed release.

Generate one (once):

```bash
mkdir -p ~/.config/muninn-release && chmod 700 ~/.config/muninn-release
python3 -c "import secrets; print(secrets.token_hex(24))" > ~/.config/muninn-release/password.txt
keytool -genkeypair -v \
  -keystore ~/.config/muninn-release/release.keystore \
  -alias muninn -keyalg RSA -keysize 4096 -validity 10000 \
  -storepass:file ~/.config/muninn-release/password.txt \
  -keypass:file ~/.config/muninn-release/password.txt \
  -dname "CN=Muninn, OU=Android, O=io.github.d3adp0rt, C=AM"
```

Set repository secrets:

| Secret | Value |
|---|---|
| `RELEASE_KEYSTORE_B64` | `base64 -w0 release.keystore` |
| `RELEASE_KEYSTORE_PASSWORD` | contents of `password.txt` |
| `RELEASE_KEY_ALIAS` | `muninn` |
| `RELEASE_KEY_PASSWORD` | contents of `password.txt` |

The current signing certificate fingerprint (SHA-256):

```
163dbdc4aea845f0e0fa25dca746cc6ff24af256f2500097e2901dd8ee17479e
```

## Publish

```bash
git tag -a v0.1.0 -m "Muninn 0.1.0"
git push origin v0.1.0
```

The workflow builds Rust for `arm64-v8a`, signs the APK with the release key,
writes a `.sha256` file and publishes both with generated release notes. It
fails early if `RELEASE_KEYSTORE_B64` is missing, so a debug-signed APK can
never be published by accident.

Version name comes from the tag without the leading `v`; version code comes
from the workflow run number.

## Verify

```bash
sha256sum -c muninn-v0.1.0-arm64-v8a.apk.sha256
apksigner verify --print-certs muninn-v0.1.0-arm64-v8a.apk
```

F-Droid builds from source with its own signing key; the GitHub APK is signed
by the maintainer key above.
