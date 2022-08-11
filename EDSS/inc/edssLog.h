#pragma once

#include <stdio.h>
#include <stdlib.h>
#include <time.h>

/// Macros for EDSS logging

// https://gist.github.com/RabaDabaDoba/145049536f815903c79944599c6f952a
#define COLOUR_BLACK "\e[40m"
#define COLOUR_RED "\e[41m"
#define COLOUR_GREEN "\e[42m"
#define COLOUR_YELLOW "\e[43m"
#define COLOUR_BLUE "\e[44m"
#define COLOUR_MAGENTA "\e[45m"
#define COLOUR_CYAN "\e[46m"
#define COLOUR_WHITE "\e[47m"
#define COLOUR_RESET "\e[0m"
#define COLOUR_BLACKTEXT "\e[0;30m"

#define TIME_SIZE 40 // %Y-%m-%dT%H:%M%SZ

char *logged_time;

char *log_time() {
    char *buf;
    time_t now;
    struct tm *localnow;
    int ret;

    buf = (char *)malloc(TIME_SIZE);
    time(&now);
    localnow = localtime(&now);

    if ((ret = strftime(buf, TIME_SIZE, "%Y-%m-%dT%H:%M:%SZ", localnow))) {
        return buf;
    }
    // This part should never happen
    perror("strftime");
    return NULL;
}

// Inspired from
// https://stackoverflow.com/questions/15549893/modify-printfs-via-macro-to-include-file-and-line-number-information

// level contains both colour and level name
#define EDSS_LOG(level, format, ...)                                           \
    logged_time = log_time();                                                  \
    printf("[" COLOUR_BLACKTEXT COLOUR_GREEN "EDSS" COLOUR_RESET               \
           " " COLOUR_BLACKTEXT level COLOUR_RESET " %s %s:%d] " format,       \
           logged_time, __FILE__, __LINE__, ##__VA_ARGS__);                    \
    free(logged_time)

#define EDSS_LOGE(format, ...)                                                 \
    EDSS_LOG(COLOUR_RED "ERROR", format, ##__VA_ARGS__)

#define EDSS_LOGD(format, ...)                                                 \
    EDSS_LOG(COLOUR_MAGENTA "DEBUG", format, ##__VA_ARGS__)

#define EDSS_LOGI(format, ...)                                                 \
    EDSS_LOG(COLOUR_BLUE "INFO", format, ##__VA_ARGS__)

#define EDSS_LOGT(format, ...)                                                 \
    EDSS_LOG(COLOUR_CYAN "TRACE", format, ##__VA_ARGS__)
