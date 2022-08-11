#pragma once

/** @file
 * Header file for EDSS error handling and related functions.
 */

/**
 * Status enum that allows callees to understand why a call failed.
 */
typedef enum {
    EDSS_OK,
    /// Invalid server configuration, excluding CAL options
    EDSS_INVALID_CONFIG,
    /// Invalid CAL options
    EDSS_INVALID_CAL_OPTIONS,
    /// When one attempts to start streaming without initialising
    /// EDSS
    EDSS_UNINITIALISED,
    /// When CAL fails to init/close because of a missing file.
    EDSS_CAL_FILE_NOT_FOUND,
    /// When CAL cannot initialize/close due to a library failure.
    EDSS_CAL_LIBRARY_FAILURE,
    // When the CAL library cannot be opened/is not defined properly.
    EDSS_INVALID_CAL,
    /// When EDSS fails because of libav generically.
    EDSS_LIBAV_FAILURE,
    /// When EDSS fails because it cannot encode a frame.
    EDSS_ENCODE_FAILURE,
    /// When EDSS fails because a data type cannot be initialized/allocated
    EDSS_ALLOCATION_FAILURE,
    /// When EDSS encounters an error with pthreads.
    EDSS_PTHREAD_FAILURE,
    /// When EDSS encounters an error with StrMap.
    EDSS_STRMAP_FAILURE,
} EDSS_STATUS;
