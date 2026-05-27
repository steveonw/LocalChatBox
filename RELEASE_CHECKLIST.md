# LocalChatBox v0.3.1 release checklist

## Before tagging

- [ ] `npm ci`
- [ ] `npm run build`
- [ ] `cd src-tauri`
- [ ] `cargo generate-lockfile` if `Cargo.lock` does not exist
- [ ] `cargo check --locked`
- [ ] Commit `src-tauri/Cargo.lock`
- [ ] Add at least one runtime binary or configure `LOCALCHATBOX_RUNTIME_PACK_URL`
- [ ] Put a small GGUF model in `models/` for local testing, but do not commit it
- [ ] Run `.\scripts\build-windows-installer.ps1`
- [ ] Install `dist-release\LocalChatBoxSetup.exe` on a clean Windows user account
- [ ] Confirm first-run setup opens
- [ ] Confirm runtime probe sees the bundled runtime
- [ ] Confirm model scan sees an imported GGUF
- [ ] Confirm Doctor result explains missing pieces or says ready
- [ ] Confirm chat works locally
- [ ] Confirm closing the app stops the runtime

## Tag

```powershell
git tag v0.3.1
git push origin v0.3.1
```

## After release

- [ ] Download `LocalChatBoxSetup.exe` from the GitHub Release
- [ ] Verify SHA256
- [ ] Install on a second clean machine or VM
- [ ] Confirm no Node/Rust/Git requirements for the normal user
