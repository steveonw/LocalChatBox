# Windows runtime binaries

Put llama.cpp `llama-server` executables here.

Recommended filenames:

| Filename | Purpose | Required? |
|---|---|---|
| `llama-server-cuda.exe` | NVIDIA CUDA runtime | Optional |
| `llama-server-cpu-avx2.exe` | CPU runtime for AVX2/FMA-era CPUs | Optional |
| `llama-server-cpu-avx.exe` | CPU runtime for older AVX-only CPUs, such as Sandy/Ivy Bridge | Optional |
| `llama-server-cpu-basic.exe` | Safest CPU fallback, built for the oldest CPUs you want to support | Recommended |
| `llama-server.exe` | Generic fallback | Optional |

For development, you do **not** need every runtime binary. At minimum, add one working runtime. The safest first test is usually `llama-server-cpu-basic.exe`.

In `auto` mode, LocalChatBox checks CPU features before choosing AVX2 or AVX. It only tries CUDA automatically when an NVIDIA GPU is detected and GPU offload is requested.

## Pin one runtime family

For a distributable build, use one pinned llama.cpp release/commit for all runtime binaries and record it in a local file such as:

```text
runtime/win/VERSION.txt
```

A template is provided as `VERSION.example.txt`.

This matters because command-line flags can move over time. Current llama.cpp documents `--ui/--no-ui`; older builds may use or document `--webui/--no-webui`. LocalChatBox v0.1.2 tries both forms.

## Flag check

After adding a binary, run:

```powershell
.\scripts\check-runtime-flags.ps1 .\runtime\win\llama-server-cpu-basic.exe
```

The script checks `llama-server --help` output for:

- `--api-key`
- `--parallel`
- `--ui` / `--no-ui`
- `--webui` / `--no-webui`

These binaries are not included in this source zip because the correct build depends on CPU instructions, GPU/CUDA compatibility, target architecture, and your license/distribution review.
