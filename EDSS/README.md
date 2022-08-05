# vgpuCapture

Admittedly, this code is a mess.
Relevant code 

- `capture.c` → capture function implementation
- `capture.h` → capture function signatures, TODO (soon) structs throughout the program
- `vgpuVideo.c` → implementations of functions and main function
- `vgpuVideo.h` → structs used throughout the program (missing function signatures for the corresponding C file)

vGPU must be initialized with `intervaltime=16000` for 60fps, otherwise the framebuffer will only refresh slow enough for 12fps capture.

## Test video streaming

After starting the RTP server: `mpv --no-cache --untimed --no-demuxer-thread --video-sync=audio --vd-lavc-threads=1 --hwdec=nvdec <SDP>` (change `nvdec` if you do not run an NVIDIA GPU)

Launching the streaming program: `sudo ./main_multithread <VGPU_ID> <CLIENT_IP>:<PORT> <SRTP_OUT_PARAMS>`

We will obviously move this into a client in the future, but this command seems to deliver latency similar to or slightly better than NoMachine, so it is good for testing/understanding.

Refer to FFmpeg protocol documentation for `srtp_out_params`.
