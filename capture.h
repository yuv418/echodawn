#pragma once

#include <pthread.h>

typedef struct {
	pthread_mutex_t mutex;
	void* buffer;
} captureData_t;

void captureThreadFunction(void *threadArgs);

