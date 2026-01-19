@echo off
setlocal

echo Starting llama-server build for Windows...

rem Get the directory of this script
set SCRIPT_DIR=%~dp0
set LLAMA_CPP_DIR=%SCRIPT_DIR%llama.cpp
set TARGET_DIR=%SCRIPT_DIR%

rem Assumes build environment is set up (e.g., in a Visual Studio Command Prompt)
rem and CMake is in the PATH.

rem Go to llama.cpp directory
cd /D "%LLAMA_CPP_DIR%"

rem Clean and build using CMake
echo Configuring and building llama.cpp...
if exist build rmdir /s /q build
mkdir build
cd build

rem Configure the build
cmake ..

rem Build the Release version
cmake --build . --config Release

rem Find the server binary
set SERVER_BIN_PATH=bin\Release\server.exe
if not exist "%SERVER_BIN_PATH%" (
    echo Server binary not found at %SERVER_BIN_PATH% after build!
    exit /b 1
)
echo Build successful. Server binary found at: %SERVER_BIN_PATH%

rem Name for Windows x64
set TARGET_TRIPLE=x86_64-pc-windows-msvc
set FINAL_NAME=llama-server-%TARGET_TRIPLE%.exe
set FINAL_PATH=%TARGET_DIR%%FINAL_NAME%

echo Copying binary to %FINAL_PATH%
copy "%SERVER_BIN_PATH%" "%FINAL_PATH%"

echo ----------------------------------------
echo Build complete!
echo Binary created at: %FINAL_PATH%
echo You can now build your Tauri app for Windows.
echo ----------------------------------------
