# Release runbook

Phase 12. Steps marked **⚠ credentials required** cannot be completed in this
repository as it stands; they need an Apple Developer ID and a Windows code
signing certificate held by the project owner. See `docs/project-status.md`.

## 1. Pre-flight

```bash
npm run check:all      # typecheck, eslint, vitest, cargo test, fmt, clippy
```

- [ ] Every check green on macOS and Windows
- [ ] `docs/manual-test-plan.md` executed on both platforms, results recorded
- [ ] `cargo audit` and `npm audit` reviewed; each finding fixed or accepted in writing
- [ ] `docs/traceability.md` updated: no requirement newly Deferred without an ADR
- [ ] Version bumped in `package.json`, `Cargo.toml` workspace, `tauri.conf.json` (all three must match)
- [ ] Release notes drafted: implemented / experimental / known limitations / privacy effects
- [ ] Benchmark report regenerated; no unverified savings claim anywhere in copy or UI

## 2. Build

```bash
npm ci
npm run build          # typecheck + tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`.

- [ ] macOS: `.app` and `.dmg` for aarch64 and x86_64
- [ ] Windows: `.msi` and/or `.exe` for x86_64
- [ ] Record the toolchain: `rustc --version`, `node --version`, Xcode / MSVC version

## 3. Sign and notarize — ⚠ credentials required

macOS:

```bash
# Requires APPLE_CERTIFICATE, APPLE_SIGNING_IDENTITY, APPLE_ID,
# APPLE_PASSWORD (app-specific), APPLE_TEAM_ID in the environment.
npm run tauri build -- --target universal-apple-darwin
xcrun stapler validate "src-tauri/target/release/bundle/macos/LeanAI Desktop.app"
spctl --assess --type execute --verbose "…/LeanAI Desktop.app"
```

Windows: sign the MSI/EXE with the organisation certificate, then
`signtool verify /pa /v <artifact>`.

- [ ] Hardened runtime enabled; entitlements reviewed (no unnecessary ones)
- [ ] Any bundled sidecar is signed and notarized **with** the app (Phase 6)
- [ ] Gatekeeper assessment passes on a machine that has never seen the app

## 4. Verify on clean machines

- [ ] Fresh macOS VM: install → open a project → bundle → save → quit → reopen
- [ ] Fresh Windows VM: same
- [ ] Upgrade from the previous release: settings, projects and presets survive; migrations apply
- [ ] Uninstall: app removed; app-data removal offered and honoured

## 5. Publish

- [ ] Tag `v<version>`, push the tag
- [ ] Attach artifacts and checksums (`shasum -a 256`) to the release
- [ ] Publish the updater manifest, signed with the updater key — ⚠ credentials required
- [ ] Staged rollout: 10 % → 50 % → 100 %, watching crash reports between stages
- [ ] Archive delivery evidence: CI run, test results, benchmark report, manual test record

## 6. Rollback

1. Halt the rollout at the current stage.
2. Revert the updater manifest to the previous version and re-sign it.
3. Publish an advisory naming the affected versions and the symptom.
4. If user data may be affected, ship a migration or a documented recovery step —
   never instruct users to delete their app-data directory without saying what
   is lost.
5. Post-incident: write the regression test first, then the fix.
