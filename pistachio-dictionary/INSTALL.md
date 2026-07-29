# Installing Pistachio Dictionary (开心果词典)

Pistachio Dictionary is a fully offline Chinese–English dictionary. Download
the installer for your OS from the GitHub **Releases** page, install, and run —
no other software is needed. The dictionary database is bundled; no internet
connection is required for looking up words.

---

## For users

### Windows

1. Download `Pistachio Dictionary_x.x.x_x64-setup.exe` (NSIS installer) from
   the latest release. (An `.msi` is also attached for managed deployments.)
2. **Expected warning:** the app is currently *unsigned*, so Windows
   SmartScreen will show "Windows protected your PC".
   Click **More info → Run anyway**. This is normal for unsigned apps and will
   disappear once we add a code-signing certificate (see *Roadmap* below).
3. Run the installer, then launch 开心果词典 from the Start menu.

### macOS

1. Download the `.dmg` matching your Mac:
   - `aarch64` → Apple Silicon (M1/M2/M3/M4)
   - `x64` → Intel
2. Open the `.dmg` and drag **Pistachio Dictionary** into **Applications**.
3. **Expected warning:** because the app is not yet signed/notarized, macOS
   may say it "cannot be opened because the developer cannot be verified".
   Fix: right-click the app in Applications → **Open** → **Open** in the dialog
   (only needed once). If macOS says the app is "damaged", run:
   `xattr -d com.apple.quarantine "/Applications/Pistachio Dictionary.app"`
4. Launch normally afterwards.

### Linux (community builds)

Linux builds (`.deb`, `.AppImage`) are not part of the primary release matrix
but the project builds on Linux unchanged (`npm run tauri build`). Windows and
macOS are the supported targets for now.

### Updates

The app checks for updates on startup. When a new release is published, it
downloads and installs itself in the background and offers a **Restart now**
button. No manual re-download is needed on any device after the first install.

---

## For maintainers: cutting a release

Prerequisites (one-time):

1. **Repo secret for update signing.** In the GitHub repo go to
   *Settings → Secrets and variables → Actions → New repository secret*:
   - Name: `TAURI_SIGNING_PRIVATE_KEY`
   - Value: the full contents of the updater private key file
     (`updater-private-key.txt`, delivered separately — **never commit it**).
   - If you lose this key, existing installs can never auto-update again and a
     new keypair must be shipped in a manual release.

2. **Set the updater endpoint.** In `src-tauri/tauri.conf.json`, replace
   `YOUR-GITHUB-USERNAME` in `plugins.updater.endpoints` with your GitHub
   username or org. The release workflow fails early if this is forgotten.

Release steps:

```bash
# 1. bump "version" in src-tauri/tauri.conf.json (and Cargo.toml), commit
# 2. tag and push
git tag v0.1.0
git push origin main --tags
```

3. GitHub Actions builds Windows (NSIS `.exe` + `.msi`) and macOS (`.dmg` for
   Apple Silicon and Intel) in parallel and creates a **draft release** with
   all installers plus the signed `latest.json` update manifest.
4. Review the draft release, then **Publish**. Existing installs pick the
   update up automatically.

Every push to any branch also runs the **Build check** workflow (frontend
build + `cargo check`), so compile errors surface long before release day.

---

## Roadmap: code signing (removes the first-run warnings)

Current state: **unsigned** — the warnings above are expected and documented
for users. To remove them we need to apply for:

| Platform | Requirement | Approx. cost | Effect |
|---|---|---|---|
| macOS | Apple Developer Program | US$99/year | Signed + notarized app; no Gatekeeper warning |
| Windows | Code-signing certificate (OV or EV) | ~US$100–400/year | SmartScreen trusts the app (EV removes warnings immediately; OV builds reputation over time) |

When certificates are obtained, add them to the release workflow
(`APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY`, notarization credentials on
macOS; `WINDOWS_CERTIFICATE` + thumbprint on Windows) — the workflow has been
structured so this is an additive change, not a rewrite.

---

## Licensing note

Dictionary data: [CC-CEDICT](https://cc-cedict.org/wiki/), Creative Commons
Attribution-ShareAlike. Attribution is shown in the app; any improvements made
to the dictionary data itself must be shared under the same license.
