#!/bin/bash
# This script builds the llama-server binary from the llama.cpp submodule.
# It's designed to be run on Linux or macOS.

set -e
echo "Starting llama-server build..."

# Get the directory of this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
LLAMA_CPP_DIR="$SCRIPT_DIR/llama.cpp"
TARGET_DIR="$SCRIPT_DIR"

# Determine OS and Arch for naming the binary
OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH_NAME=$(uname -m)

TARGET_TRIPLE=""

if [ "$OS_NAME" == "darwin" ]; then
    if [ "$ARCH_NAME" == "arm64" ]; then
        TARGET_TRIPLE="aarch64-apple-darwin"
    else
        TARGET_TRIPLE="x86_64-apple-darwin"
    fi
elif [ "$OS_NAME" == "linux" ]; then
    TARGET_TRIPLE="x86_64-unknown-linux-gnu" # Assuming common case
else
    echo "Unsupported OS: $OS_NAME. This script is for Linux and macOS."
    exit 1
fi

echo "Detected target: $TARGET_TRIPLE"

# Go to llama.cpp directory
cd "$LLAMA_CPP_DIR"

# Clean and build using CMake
echo "Configuring and building llama.cpp..."
rm -rf build
mkdir build
cd build

# Add -DLLAMA_METAL=ON for Apple Silicon for better performance
if [ "$TARGET_TRIPLE" == "aarch64-apple-darwin" ]; then
    cmake .. -DLLAMA_METAL=ON
else
    cmake ..
fi

cmake --build . --config Release

# Find the server binary
SERVER_BIN_PATH="bin/server"
if [ ! -f "$SERVER_BIN_PATH" ]; then
    echo "Server binary not found at $SERVER_BIN_PATH after build!"
    exit 1
fi
echo "Build successful. Server binary found at: $SERVER_BIN_PATH"

# Copy and rename the binary to the binaries directory
FINAL_NAME="llama-server-$TARGET_TRIPLE"
FINAL_PATH="$TARGET_DIR/$FINAL_NAME"

echo "Copying binary to $FINAL_PATH"
cp "$SERVER_BIN_PATH" "$FINAL_PATH"

echo "----------------------------------------"
echo "Build complete!"
echo "Binary created at: $FINAL_PATH"
echo "You can now build your Tauri app for this platform."
echo "----------------------------------------"
