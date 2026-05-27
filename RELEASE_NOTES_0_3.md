# LocalChatBox v0.3 release notes

## Theme

v0.3 is the "Switchboard Installer Preview" release.

The app now points toward the actual end-user goal:

```text
Install → setup wizard → hardware scan → runtime check → model fit → chat
```

## Highlights

- First-run setup flow.
- Runtime engine manifest.
- Automatic llama-server flag probe.
- Router-mode detection.
- Generated router preset.
- Model registry with fit labels.
- Doctor diagnostics report.
- Runtime startup cancellation generation.
- Local API key moved to `LLAMA_API_KEY`.
- HTTP timeouts.
- Settings validation and recovery.
- Tauri CSP enabled.
- NSIS/MSI installer config prepared.

## Validation

Passed:

```powershell
npm install --ignore-scripts
npm run build
npm audit --omit=dev --json
```

Not performed in this environment:

```powershell
cargo check
npm run tauri:dev
npm run tauri:build
```

Reason: `cargo` is unavailable in the build environment.

## Next required Windows step

Run:

```powershell
cd LocalChatBox
npm install
cd src-tauri
cargo check
cd ..
npm run tauri:dev
```

Then test with:

```text
runtime/win/llama-server-cpu-basic.exe
models/<small-test-model>.gguf
```
