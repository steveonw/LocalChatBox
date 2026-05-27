# LocalChatBox v0.3

LocalChatBox is a Windows-first local AI chat cockpit for `.gguf` models.

The end-user promise for the finished installer is:

```text
Download LocalChatBoxSetup.exe
Double-click installer
Open LocalChatBox
Follow first-run setup
Pick a model that fits
Chat locally
```

The current package is **v0.3 source / installer-preview**, not a signed public installer. It is the next architectural build after v0.1.2: the app now treats runtime probing, model registry, diagnostics, and installer-first setup as core product features instead of README chores.

---

## What v0.3 adds

### Runtime Switchboard

LocalChatBox now creates a llama.cpp engine manifest:

```text
data/engines/llama.cpp/manifest.json
```

It probes available runtime binaries in:

```text
runtime/win/
```

and records:

- which binaries exist,
- whether `--help` runs,
- detected version string,
- supported flags,
- current/legacy UI-disable flag,
- whether router mode appears supported.

Known runtime filenames:

```text
llama-server-cuda.exe
llama-server-cpu-avx2.exe
llama-server-cpu-avx.exe
llama-server-cpu-basic.exe
llama-server.exe
```

### Router-mode detection

If a verified runtime exposes `--models-preset` and `--models-max`, v0.3 can launch in llama.cpp router mode and generate:

```text
data/engines/llama.cpp/router.preset.ini
```

The router preset is generated from the model registry. When router mode is unavailable, LocalChatBox falls back to classic single-model mode.

### Model Registry

The old raw `.gguf` scan is now promoted into:

```text
data/models/registry.json
```

Each model gets:

- stable local model ID,
- display name,
- family / quantization hints,
- size,
- estimated RAM need,
- recommended context,
- recommended GPU layers,
- fit status:
  - `Fits`
  - `May be slow`
  - `Risky`
  - `Won’t fit`
  - `Unknown`
- plain-English reasons.

This is still a heuristic. A real GGUF metadata parser remains a future improvement.

### Doctor report

The app now writes:

```text
data/diagnostics/doctor-report.json
```

The Doctor checks:

- hardware scan,
- verified runtimes,
- router support,
- model count,
- model fit status,
- next recommended action.

### First-run setup UI

The app now opens with a setup path instead of dropping the user directly into an empty chat screen:

1. privacy note,
2. hardware scan,
3. runtime check,
4. model scan,
5. doctor report,
6. start/load model.

### Hardening from the deep audit

v0.3 also patches several v0.1.2 problems:

- chat/settings drafts are preserved during status polling,
- runtime startup has cancellation generation tracking,
- duplicate startup attempts kill the previous child before launching,
- app close attempts to stop the runtime,
- local API key is passed through `LLAMA_API_KEY`, not command-line args,
- local chat requests use the backend loaded model ID,
- HTTP requests have explicit timeouts,
- malformed settings are backed up and reset,
- model tier matching no longer lets `14B` match `4B`,
- Tauri CSP is no longer disabled,
- runtime logs remain parent-captured to avoid Windows dual-writer contention.

---

## What is not included

This source package does **not** include:

- signed Windows installer artifact,
- bundled llama.cpp runtime binaries,
- bundled GGUF model,
- one-click model downloader,
- code signing certificate,
- native Tauri/Rust build validation in this environment,
- streaming token UI,
- GGUF header parser,
- RAG/tools/voice/agents.

The source is installer-prepared, but a real end-user release still needs a Windows build machine and runtime/model licensing decisions.

---

## Normal-user target flow

The planned public flow is:

```text
LocalChatBoxSetup.exe
  ↓
First launch setup
  ↓
Hardware scan
  ↓
Bundled CPU runtime verified
  ↓
Beginner model import/download
  ↓
Fits / May be slow / Won’t fit guidance
  ↓
Start chat
```

For this source build, developers still need to provide a runtime binary and model file.

---

## Developer quick start

From the project root:

```powershell
npm install
npm run build
```

To run the Tauri desktop app on Windows:

```powershell
npm run tauri:dev
```

Add at least one GGUF model:

```text
models/<your-model>.gguf
```

Add at least one runtime binary:

```text
runtime/win/llama-server-cpu-basic.exe
```

Optional acceleration/runtime variants:

```text
runtime/win/llama-server-cuda.exe
runtime/win/llama-server-cpu-avx2.exe
runtime/win/llama-server-cpu-avx.exe
runtime/win/llama-server.exe
```

Then open the app and use:

```text
Setup → Scan Hardware
Setup → Check Runtime
Setup → Scan Models
Setup → Run Doctor
Chat → Start / Load Selected
```

---

## Windows installer build target

v0.3 configures Tauri bundle targets for:

```text
NSIS setup.exe
MSI
```

The intended normal-user artifact is:

```text
LocalChatBoxSetup.exe
```

Build on a Windows machine with Tauri/Rust prerequisites installed:

```powershell
npm install
npm run tauri:build
```

See:

```text
DEVELOPERS.md
docs/END_USER_INSTALL.md
```

for the installer ladder and validation checklist.

---

## Folder layout

```text
LocalChatBox/
  src/
    main.ts
    style.css
    tauri.ts
    types.ts

  src-tauri/
    src/
      main.rs
      paths.rs
      hardware.rs
      models.rs
      engine.rs
      diagnostics.rs
      runtime.rs
      storage.rs

  models/
    put-gguf-models-here.txt

  runtime/
    win/
      README.md
      VERSION.example.txt
      llama-server-cuda.exe        # developer/user supplied unless bundled by installer
      llama-server-cpu-avx2.exe
      llama-server-cpu-avx.exe
      llama-server-cpu-basic.exe
      llama-server.exe

  data/
    settings.example.json
    settings.json                  # created on first run
    hardware-profile.json          # created by hardware scan
    runtime-state.json             # created by runtime status updates
    chats.json                     # created after chat save
    engines/
      llama.cpp/
        manifest.json              # created by runtime probe
        router.preset.ini          # created before router launch
    models/
      registry.json                # created by model scan
    diagnostics/
      doctor-report.json           # created by Doctor

  logs/
    runtime.log
```

---

## Runtime mode behavior

### Auto

`runtime_mode = "auto"` chooses router mode when the selected runtime proves it supports router flags, otherwise it uses classic mode.

### Router

`runtime_mode = "router"` requires a runtime with router support. The app writes `router.preset.ini`, starts one router process, and uses `/models/load` for switches where possible.

### Classic

`runtime_mode = "classic"` starts one `llama-server` process for one selected model. Switching models restarts the runtime.

---

## Privacy contract

Local mode:

```text
Frontend → Tauri IPC → Rust backend → 127.0.0.1 llama-server
```

The frontend does not receive the generated local runtime token. Rust sends it to `llama-server` through the child-process environment variable `LLAMA_API_KEY` and injects the `Authorization` header for local requests.

Remote mode:

```text
Frontend → Tauri IPC → Rust backend → configured remote endpoint
```

Messages are sent to the endpoint you configure. The UI should make that explicit before a normal-user release.

---

## Known limitations

- Runtime probing uses `llama-server --help` and `--version`; unusual builds may behave differently.
- Router support is detected by flags, not by a live full model-switch integration test.
- Model RAM estimates are heuristic.
- The source package does not include signed binaries or model licenses.
- The WebView2 bootstrapper and installer signing need real Windows release testing.
- Native Rust/Tauri compilation was not possible in the environment that produced this package.

---

## Release ladder

```text
v0.3 source / installer-preview
  runtime manifest
  model registry
  doctor
  first-run setup
  installer config

v0.4 beta
  actual Windows setup.exe artifact
  bundled CPU runtime pack
  model import/download UX
  native build validation
  runtime smoke tests

v1.0
  signed installer
  public beginner model path
  stable privacy docs
  code-signing/reputation plan
```

LocalChatBox should be judged by the normal user’s path, not the developer’s cleverness.
