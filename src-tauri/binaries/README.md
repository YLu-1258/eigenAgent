# llama-server Sidecar Setup

Place the `llama-server` binary in this directory with the correct platform suffix.

## Required Naming Convention

Tauri sidecars require platform-specific naming:

| Platform | Filename |
|----------|----------|
| macOS Apple Silicon | `llama-server-aarch64-apple-darwin` |
| macOS Intel | `llama-server-x86_64-apple-darwin` |
| Linux x64 | `llama-server-x86_64-unknown-linux-gnu` |
| Windows x64 | `llama-server-x86_64-pc-windows-msvc.exe` |

## Getting llama-server

### Option 1: Build from source

```bash
git clone https://github.com/ggml-org/llama.cpp
cd llama.cpp
cmake -B build
cmake --build build --config Release -t llama-server
```

The binary will be at `build/bin/llama-server`.

### Option 2: Download pre-built

Download from llama.cpp releases: https://github.com/ggml-org/llama.cpp/releases

## Example (macOS Apple Silicon)

```bash
# After building or downloading
cp /path/to/llama-server ./llama-server-aarch64-apple-darwin
chmod +x ./llama-server-aarch64-apple-darwin
```

## Vision Support

For vision models like Qwen3-VL, you also need the multimodal projector file (`*mmproj*.gguf`) in your `models/` directory alongside the main model.
