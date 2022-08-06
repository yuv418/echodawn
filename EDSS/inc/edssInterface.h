#pragma once

#include "../vendor/strmap/strmap.h"
#include "edssStatus.h"
#include <arpa/inet.h>

/** @file
 * Configuration enum for control server to set, influenced by ED client.
 */

/**
 * EDSS configuration sent from control server which will be taken into account
 * for when the stream begins. EDSS streams can also be adjusted on-the-fly from
 * this struct.
 */
typedef struct {
    struct in_addr ip; // IPv4 is a 32 bit int
    uint16_t port;     // max 65535
    uint32_t bitrate;
    uint32_t framerate;
    char srtpOutParams[30]; // Rust control to will specify these
    StrMap *calOptionDict;  // Sent to CAL

    // NOTE that some things like
    // width/height will get set here.
    // Only some CALs can support changing
    // Width/height.
} edssConfig_t;

/**
 * Initialize the server. This will allocate and initialize various FFmpeg
 * structures with the values provided from the provided `cfg` variable.
 * Furthermore, any capture abstraction libraries will have their initializers
 * called, for example initing Xlib, PipeWire, or acquiring a framebuffer. */
EDSS_STATUS edssInitServer(edssConfig_t *cfg);

/** Begin the SRTP server main loop, and begin the capture thread. NOTE that
 * this function blocks. TODO make this function not block by running the
 * encoder thread automatically. */
EDSS_STATUS edssInitStreaming();

/** Stop the SRTP server main loop, and stop the capture thread. */
EDSS_STATUS edssCloseStreaming();

/**
 * Update the SRTP stream's to reflect whatever the values are in
 * the edssConfig_t config pointer. */
EDSS_STATUS edssUpdateStreaming();
/**
 * Capture abstraction libraries (CALs) may expose options to the client which
 * they can set. This function allows the control server to retrieve CAL options
 * so they can be set before `edInitServer` is called.
 *
 * The first function to call when using EDSS. This will open the CAL plugin
 * name as a file and ready it for edssInitServer. Furthermore, this will return
 * the option dict for you to configure CAL properly.
 */
EDSS_STATUS edssOpenCAL(char calPluginName[100], StrMap *calOptionDict);
