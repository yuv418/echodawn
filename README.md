# vgpuCapture

Admittedly, this code is a mess.
Relevant code 

- `capture.c` → capture function implementation
- `capture.h` → capture function signatures, TODO (soon) structs throughout the program
- `vgpuVideo.c` → implementations of functions and main function
- `vgpuVideo.h` → structs used throughout the program (missing function signatures for the corresponding C file)

vGPU must be initialized with `intervaltime=16000` for 60fps, otherwise the framebuffer will only refresh slow enough for 12fps capture.
