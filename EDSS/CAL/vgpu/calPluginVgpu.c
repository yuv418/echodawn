#include "../../inc/edssCALInterface.h"
#include "../../inc/edssLog.h"
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>
#include <unistd.h>

// This is a not a good solution. This is unfortunately set by the guest so we
// can't really do much but hardcode for now. There may be ways to use RM to
// get the width/height, so we will use these in the future.
#define IMGBUF_WIDTH 1920
#define IMGBUF_HEIGHT 1080
#define VGPU_MMAP_CONSOLE_OFFSET 0x10000000000

struct vgpuCALRTCfg {
    int vgpuFd;
    calConfig_t *calCfg;
};

static struct vgpuCALRTCfg rtCfg;

EDSS_STATUS calOptions(StrMap **calOptionDict) {
    *calOptionDict = sm_new(1);
    if (!sm_put(*calOptionDict, "vgpuId", "")) {
        return EDSS_STRMAP_FAILURE;
    }

    return EDSS_OK;
}

EDSS_STATUS calInit(StrMap *calOptionDict, calConfig_t *calCfg) {

    char gid_path[32]; // QEMU uses this as the max size for a vfio device, so..
                       // we will too Why do we care about QEMU though?
    int vgpuFd;

    char vgpuIdValueCh[4]; // I'm sure someone wants to run 1000 vGPU VMs on one
                           // host
    int vgpuIdValue;

    sm_get(calOptionDict, "vgpuId", vgpuIdValueCh, sizeof(vgpuIdValueCh));
    vgpuIdValue = atoi(vgpuIdValueCh);

    snprintf(gid_path, sizeof(gid_path), "/dev/nvidia-vgpu%d", vgpuIdValue);

    vgpuFd = open(gid_path, O_RDWR);
    if (vgpuFd < 0) {
        EDSS_LOGE("open /dev/nvidia-vgpu%d: %s\n", vgpuIdValue,
                  strerror(errno));
        return EDSS_CAL_FILE_NOT_FOUND;
    }

    rtCfg.vgpuFd = vgpuFd;
    rtCfg.calCfg = calCfg;
    rtCfg.calCfg->frame =
        (uint8_t *)mmap(0, IMGBUF_WIDTH * IMGBUF_HEIGHT * 4, PROT_READ,
                        MAP_PRIVATE, rtCfg.vgpuFd, VGPU_MMAP_CONSOLE_OFFSET);
    if (rtCfg.calCfg->frame < 0) {
        return EDSS_CAL_LIBRARY_FAILURE;
    }

    rtCfg.calCfg->width = IMGBUF_WIDTH;
    rtCfg.calCfg->height = IMGBUF_HEIGHT;
    rtCfg.calCfg->pixFmt = AV_PIX_FMT_BGRA;

    // TODO extract `intervaltime` from
    // `/sys/bus/mdev/devices/<UUID>/nvidia/vgpu_params`
    rtCfg.calCfg->framerate = 60;

    return EDSS_OK;
}

EDSS_STATUS calReadFrame() {
    // We don't have do anything since the framebuffer pointer is already
    // auto-updated. In other CALs we might have to fetch a frame and copy it
    // to calCfg.
    return EDSS_OK;
}

EDSS_STATUS calShutdown() {
    int ret;

    ret = close(rtCfg.vgpuFd);
    if (ret < 0) {
        return EDSS_CAL_LIBRARY_FAILURE;
    }

    rtCfg.calCfg->frame = NULL;
    rtCfg.calCfg->height = 0;
    rtCfg.calCfg->width = 0;
    rtCfg.calCfg->pixFmt = 0;

    return EDSS_OK;
}

calPlugin_t calPlugin = {
    .calOptions = calOptions,
    .calInit = calInit,
    .calReadFrame = calReadFrame,
    .calShutdown = calShutdown,
};
