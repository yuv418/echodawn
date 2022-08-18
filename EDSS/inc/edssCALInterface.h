#pragma once

#include "../vendor/strmap/strmap.h"
#include "edssInterface.h"
#include "edssStatus.h"
#include <libavutil/pixfmt.h>
#include <stdint.h>
#include <stdlib.h>

/** @file
 * Header file for capture abstraction libraries (CALs). Capture abstraction
 * libraries plug into the ED streaming server using dlopen. These CALs
 * furthermore feed the capture thread frames. The following is a header file of
 * the functions that CALs must implement to function in EDSS.
 */

/**
 * CAL config struct. NOTE that CAL config != CAL options. This is for the
 * streaming server to understand the CAL better. CAL options are set by the
 * client.
 */
typedef struct {
    enum AVPixelFormat pixFmt;
    /**
     * CAL config tells EDSS how big the frame width is.
     */
    uint16_t height;
    /**
     * CAL config tells EDSS how big the frame height is.
     */
    uint16_t width;
    /// The framerate of the video.
    uint16_t framerate;
    /** The `uint8_t*` is provided without a size since we are not aware of the
     * size of the frame without looking at CAL config (plus it is more
     * efficient to re-use the frame pointer than allocate a new frame every
     * capture cycle). This variable is allocated by CAL.
     */
    uint8_t *frame;
} calConfig_t;

/**
 * CAL plugin structure. Define the functions here for the CAL plugin to work.
 */
typedef struct {

    /**
     * Called by `edssGetCALOptionsDict`. Effectively the "implementation" for
     * that function. See documentation in edssInterface.h.
     */
    EDSS_STATUS (*calOptions)(StrMap **);

    /**
     * Initialize CAL (eg. init Xlib/etc). This should be set up such that when
     * `calReadFrame` is run, no more initialization is done. We want to
     * maximise the speed of reading frames. This function will also return the
     * CAL configuration for the frame type (eg. BGRA, RGBA, YUV, whatever) so
     * EDSS can act accordingly when deciding how to process and encode frames.
     * For example, if YUV frames are captured, EDSS can skip the step of
     * converting pixel formats to YUV.
     */
    EDSS_STATUS (*calInit)(StrMap *, calConfig_t *);

    /**
     * Read a frame from CAL. This should be in the pixFmt provided in CAL
     * config. The resulting frame is stored in calConfig's frame pointer.
     */
    EDSS_STATUS (*calReadFrame)();
    /**
     * Write a mouse event to CAL.
     */
    EDSS_STATUS (*calWriteMouseEvent)(edssMouseEvent_t *ev);

    /**
     * Free memory that was allocated in `calInit`. For example, close files or
     * free any handles that are being used.
     */
    EDSS_STATUS (*calShutdown)();

    // TODO add an "update configuration" handler
} calPlugin_t;
