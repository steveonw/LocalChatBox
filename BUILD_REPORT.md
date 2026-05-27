# LocalChatBox v0.3 build report

## Package

Version: `0.3.0 source / installer-preview`

This package was built from the previous v0.1.2 source project and patched around the agreed v0.3 plan:

```text
Router Runtime + Model Registry + Installer-First Experience
```

## Important artifact status

This is a **source package**, not a finished signed Windows installer.

Included:

- Tauri v2 app source
- Rust backend source
- Vite/TypeScript frontend source
- installer-oriented Tauri config for NSIS/MSI targets
- model registry support
- engine manifest support
- doctor diagnostics support
- runtime hardening patches
- first-run setup UI

Not included:

- compiled `.exe`
- signed installer
- llama.cpp runtime binaries
- GGUF model files
- code-signing certificate
- native Rust/Tauri validation from this environment

## v0.3 changes implemented

### Frontend

- Added setup-first user flow:
  - privacy note
  - hardware scan
  - runtime probe
  - model scan
  - doctor report
  - mark setup complete
- Added Model Registry tab.
- Added Engine Manager tab.
- Added clearer runtime strip in Chat.
- Added Switch Model action.
- Preserves chat/system prompt drafts during polling.
- Skips poll-triggered full re-render while an input/textarea/select is focused.
- Uses backend `loaded_model_id` where possible.

### Rust backend

New modules:

```text
src-tauri/src/engine.rs
src-tauri/src/diagnostics.rs
```

Patched modules:

```text
src-tauri/src/runtime.rs
src-tauri/src/models.rs
src-tauri/src/storage.rs
src-tauri/src/paths.rs
src-tauri/src/main.rs
```

New commands:

```text
scan_model_registry
probe_runtime_manifest
run_doctor
switch_local_model
```

### Engine manifest

`probe_runtime_manifest` writes:

```text
data/engines/llama.cpp/manifest.json
```

It checks known runtime filenames, runs `--help` / `--version`, records supported flags, and marks router support if the runtime exposes `--models-preset` and `--models-max`.

### Model registry

`scan_model_registry` writes:

```text
data/models/registry.json
```

Each GGUF gets a stable local ID, fit label, RAM estimate, context recommendation, and reason list.

### Router mode

Runtime startup now supports:

```text
runtime_mode = auto | router | classic
```

In `auto`, the app uses router mode when the verified runtime supports it, otherwise it falls back to classic single-model mode.

Router mode writes:

```text
data/engines/llama.cpp/router.preset.ini
```

The generated preset includes every GGUF found by the registry and sets `load-on-startup = true` for the selected model.

### Doctor report

`run_doctor` writes:

```text
data/diagnostics/doctor-report.json
```

The report gives a result code and a plain-English next step.

### Runtime hardening

- Added startup generation tracking.
- Stop Runtime cancels an active startup generation.
- New launch kills previous child before starting another.
- Runtime shutdown is attempted when the Tauri window is closed.
- Local API key is now passed via `LLAMA_API_KEY` instead of command-line args.
- Parent process remains the only runtime log writer.
- HTTP clients now use explicit connect/read timeouts.
- Local chat requests use `loaded_model_id` or backend model name instead of trusting the currently selected UI filename.
- Settings now use serde defaults, validation, clamping, and malformed-file backup/reset.
- Model tier detection checks larger parameter markers first and uses boundary-aware detection.

### Installer-first changes

- App version set to `0.3.0`.
- Tauri bundle targets set to NSIS and MSI.
- WebView2 installer mode configured as `downloadBootstrapper`.
- README now separates normal-user release goals from developer commands.
- Added end-user and developer docs.
- Added runtime/model/doctor data folders.

## Validation performed here

These commands passed:

```powershell
npm install --ignore-scripts
npm run build
```

Result:

- TypeScript compile passed.
- Vite production frontend build passed.
- New frontend source builds into `dist/`.

## Validation not performed here

The environment used to create this package does not provide `cargo`, so these were **not** performed:

```powershell
cargo check
npm run tauri:dev
npm run tauri:build
```

The following also require a Windows machine with real binaries/models:

- runtime probe against a real `llama-server.exe`,
- classic runtime launch,
- router runtime launch,
- `/models/load` switching,
- NSIS setup executable generation,
- MSI generation,
- WebView2 bootstrapper validation,
- SmartScreen/signing validation.

## Known risk register

| Risk | Status | Mitigation |
|---|---|---|
| Rust compile not validated here | Open | Run `cargo check` / `npm run tauri:dev` on Windows |
| Router preset behavior may vary by llama.cpp build | Reduced | Probe flags; keep classic fallback |
| `/models/load` integration not live-tested | Open | Test with recent llama.cpp runtime |
| Tauri close-event shutdown not Windows-job-object strong | Partial | Attempts graceful stop; Job Object still future work |
| No signed installer | Open | v0.4/v1 release task |
| No bundled runtime | Open | v0.4 release task |
| No beginner model downloader | Open | v0.4 release task after license review |
| Vite/esbuild dev-chain audit warnings | Known | Production dependency audit remains the more relevant check; update Vite chain in a later dependency pass |

## Windows validation checklist

On a Windows dev machine:

```powershell
cd LocalChatBox
npm install
npm run build
cd src-tauri
cargo check
cd ..
npm run tauri:dev
```

Then add:

```text
models/<small-test-model>.gguf
runtime/win/llama-server-cpu-basic.exe
```

Inside the app:

```text
Setup → Scan Hardware
Setup → Check Runtime
Setup → Scan Models
Setup → Run Doctor
Chat → Start / Load Selected
Chat → Send "hello"
```

Router test with a recent llama.cpp runtime:

```text
Settings → Runtime mode → router
Setup → Check Runtime
Chat → Start / Load Selected
Chat → Switch Model
```

Installer test:

```powershell
npm run tauri:build
```

Expected Windows artifacts should appear under the Tauri target release bundle folder if Rust/Tauri prerequisites are installed.
