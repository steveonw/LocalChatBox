# End-user install plan

This file describes the intended normal-user experience for LocalChatBox. The current v0.3 package is source/installer-preview; a real public release still needs a Windows build and signed installer.

## Public artifact names

Recommended:

```text
LocalChatBoxSetup.exe
```

Advanced:

```text
LocalChatBoxPortable.zip
```

Developers:

```text
LocalChatBox_v0.3_source.zip
```

## Normal user path

```text
1. Download LocalChatBoxSetup.exe.
2. Run the installer.
3. Open LocalChatBox.
4. Follow first-run setup.
5. Choose or import a beginner GGUF model.
6. Start chat.
```

The user should not need:

```text
Node
Rust
npm
cargo
Git
PowerShell scripts
manual llama.cpp flag knowledge
```

## Installer requirements for v0.4+

A public installer should bundle at least one CPU runtime:

```text
runtime/win/llama-server-cpu-basic.exe
```

Recommended additional runtime variants:

```text
runtime/win/llama-server-cpu-avx.exe
runtime/win/llama-server-cpu-avx2.exe
```

GPU acceleration should be a separate optional path until CUDA/Vulkan packaging is fully tested.

## First-run wizard

The app should show:

```text
Welcome
Privacy
Hardware scan
Runtime check
Model setup
Ready
```

The user-facing language should be:

```text
Fits
May be slow
Won't fit
```

not:

```text
n-gpu-layers
ctx-size
router.preset.ini
```

Advanced details can be expandable.

## Model policy

Do not bundle a model until its license has been reviewed.

A v0.4+ model flow can offer:

```text
Recommended small model
Balanced model
Import existing GGUF
Use remote endpoint temporarily
```

Every downloadable model must show:

```text
source
license
size
fit estimate
privacy note
```

## Signing and trust

A normal-user release should be code-signed. Unsigned installers can trigger SmartScreen warnings and damage user trust.

Release ladder:

```text
v0.3 source preview
v0.4 unsigned beta installer for testers
v0.5 signed beta if feasible
v1.0 signed public installer
```
