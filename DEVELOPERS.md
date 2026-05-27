# Developer guide

## Requirements

Install on Windows:

```text
Node.js 20+
Rust stable MSVC toolchain
Microsoft C++ Build Tools
Tauri CLI dependency chain
Microsoft Edge WebView2 Runtime if missing
```

## Commands

Install dependencies:

```powershell
npm install
```

Build frontend:

```powershell
npm run build
```

Run desktop app:

```powershell
npm run tauri:dev
```

Build installer artifacts:

```powershell
npm run tauri:build
```

## Runtime binaries

Place developer-supplied llama.cpp `llama-server` binaries in:

```text
runtime/win/
```

Known filenames:

```text
llama-server-cuda.exe
llama-server-cpu-avx2.exe
llama-server-cpu-avx.exe
llama-server-cpu-basic.exe
llama-server.exe
```

Use the helper script:

```powershell
.\scripts\check-runtime-flags.ps1 .\runtime\win\llama-server-cpu-basic.exe
```

v0.3 also probes flags automatically inside the app.

## Test matrix

Minimum local smoke test:

```text
CPU basic runtime
1B or 3B Q4 GGUF
runtime_mode = classic
```

Router smoke test:

```text
recent llama.cpp runtime with --models-preset and --models-max
two small GGUF models
runtime_mode = router
start model A
switch to model B
send chat
```

Installer smoke test:

```text
fresh Windows user profile
no Node/Rust installed
install setup.exe
open app
first-run wizard appears
bundled runtime probes successfully
model path works
```

## Native build caveat

The v0.3 source was generated in an environment where `cargo` was unavailable. Run `cargo check` before treating the source as release-ready.
