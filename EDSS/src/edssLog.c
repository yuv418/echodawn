#include "../inc/edssLog.h"

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
