# LocalChatBox release pipeline

This is the v0.3.1 pipeline goal:

```text
push to main
  -> validate frontend + Rust backend
  -> build Windows NSIS setup.exe and MSI
  -> upload workflow artifacts

tag v0.3.1
  -> require at least one runtime/win/llama-server*.exe unless explicitly overridden
  -> build installer
  -> stage LocalChatBoxSetup.exe, MSI, Portable ZIP, SHA256 files
  -> publish/update GitHub Release
```

## Files that matter

```text
.github/workflows/build-windows.yml
scripts/prepare-runtime-pack.ps1
scripts/stage-release-artifacts.ps1
scripts/build-windows-installer.ps1
src-tauri/tauri.conf.json
.gitignore
```

## Runtime pack options

For a real user-facing release, choose one of these:

### Option A: manual local build

Put at least one runtime here before building:

```text
runtime/win/llama-server-cpu-basic.exe
```

Then run:

```powershell
npm run build:installer
```

### Option B: GitHub Actions runtime pack URL

Create a ZIP containing one or more files named:

```text
llama-server-cpu-basic.exe
llama-server-cpu-avx.exe
llama-server-cpu-avx2.exe
llama-server-cuda.exe
llama-server.exe
```

Then set this GitHub repository variable:

```text
LOCALCHATBOX_RUNTIME_PACK_URL
```

The workflow downloads that ZIP and copies `llama-server*.exe` files into `runtime/win/` before building.

## Release steps

```powershell
git add .
git commit -m "Prepare v0.3.1 installer pipeline"
git push origin main
```

Then wait for the Actions build to pass.

For a release:

```powershell
git tag v0.3.1
git push origin v0.3.1
```

The workflow will publish a GitHub Release if the tag build succeeds.

## If you do not have runtime binaries yet

Use the manual workflow dispatch input:

```text
allow_runtimeless_release = true
```

That is only for source-preview/testing builds. It should not be used for public end-user releases.
