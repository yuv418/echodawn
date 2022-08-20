#pragma once

#include "../vendor/strmap/strmap.h"
#include "edssStatus.h"
#include <arpa/inet.h>
#include <stdbool.h>

/** @file
 * Configuration enum for control server to set, influenced by ED client.
 */

/**
 * EDSS configuration sent from control server which will be taken into account
 * for when the stream begins. EDSS streams can also be adjusted on-the-fly from
 * this struct.
 */
typedef struct {
    uint32_t ip;   // IPv4 is a 32 bit int so that's what it is. This is useful
                   // for cross-platform compatibility.
    uint16_t port; // max 65535
    uint32_t bitrate;
    uint32_t framerate;
    char srtpOutParams[41]; // Rust will specify these. NOTE that this should
                            // maximum be 30(?) characters + 1 for the
                            // terminator.
    StrMap *calOptionDict;  // Sent to CAL

    // NOTE that some things like
    // width/height will get set here.
    // Only some CALs can support changing
    // Width/height.
} edssConfig_t;

/// Common input data struct
typedef struct {
    int32_t button;
    bool pressed;
} edssKeyData_t;

/**
 * Mouse event struct
 */
typedef enum { CLICK, MOVE } edssMouseEventType_t;

typedef struct {
    edssMouseEventType_t type;

    union {
        struct move {
            uint32_t x;
            uint32_t y;
        } move;

        edssKeyData_t button;
    } payload;

} edssMouseEvent_t;

/**
 * Keyboard event struct
 */
typedef struct {
    edssKeyData_t keyData;
} edssKeyboardEvent_t;

/**
 * Initialize the server. This will allocate and initialize various FFmpeg
 * structures with the values provided from the provided `cfg` variable.
 * Furthermore, any capture abstraction libraries will have their initializers
 * called, for example initing Xlib, PipeWire, or acquiring a framebuffer. This
 * function takes a pointer to a char* (sdpBuffer), which will be allocated in
 * the function and used to store the resulting SDP data. The sdpBuffer will be
 * NULL if the function returned somethinng other than EDSS_OK.  */
EDSS_STATUS
edssInitServer(edssConfig_t *cfg, char **sdpBuffer);

/** Begin the SRTP server main loop, and begin the capture thread. NOTE that
 * this function blocks. TODO make this function not block by running the
 * encoder thread automatically. */
EDSS_STATUS edssInitStreaming();

/** Stop the SRTP server main loop, and stop the capture thread. */
EDSS_STATUS edssCloseStreaming();

/** Write a mouse event to the CAL */
EDSS_STATUS edssWriteMouseEvent(edssMouseEvent_t *ev);

/** Write a keyboard event to the CAL */
EDSS_STATUS edssWriteKeyboardEvent(edssKeyboardEvent_t *ev);

/**
 * Update the SRTP stream's to the new cfg pointer (we only pass a new pointer
 * since it makes Rust FFI easier). */
EDSS_STATUS edssUpdateStreaming(edssConfig_t *cfg);
/**
 * Capture abstraction libraries (CALs) may expose options to the client which
 * they can set. This function allows the control server to retrieve CAL options
 * so they can be set before `edInitServer` is called.
 *
 * The first function to call when using EDSS. This will open the CAL plugin
 * name as a file and ready it for edssInitServer. Furthermore, this will return
 * the option dict for you to configure CAL properly.
 */
EDSS_STATUS edssOpenCAL(char calPluginName[100], StrMap **calOptionDict);
