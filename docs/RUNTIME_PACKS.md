# Runtime packs

LocalChatBox v0.3 still expects runtime binaries to be supplied by developers/testers. Public installer builds should bundle at least one CPU runtime.

## Developer filenames

```text
runtime/win/llama-server-cpu-basic.exe
runtime/win/llama-server-cpu-avx.exe
runtime/win/llama-server-cpu-avx2.exe
runtime/win/llama-server-cuda.exe
runtime/win/llama-server.exe
```

## Probe behavior

The app runs:

```text
llama-server.exe --help
llama-server.exe --version
```

It records:

```text
data/engines/llama.cpp/manifest.json
```

## Router detection

Router mode is considered available when the runtime help output includes:

```text
--models-preset
--models-max
```

## API key handling

v0.3 passes the generated local key through:

```text
LLAMA_API_KEY
```

not through command-line arguments.
