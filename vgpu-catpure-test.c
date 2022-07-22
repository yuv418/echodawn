#include <linux/vfio.h>
#include <fcntl.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

#include "libbmp.h"

typedef struct {
	uint8_t b;
	uint8_t g;
	uint8_t r;
	uint8_t a;
} pixel;

int main(int argc, char **argv) {
	if (argv[1] == NULL) {
		fprintf(stderr, "usage: ./program [vfio_group_id]\n");
		return 1;
	}
	bmp_img img;
	bmp_img_init_df (&img, 1920, 1080);

	char gid_path[32]; // QEMU uses this as the max size for a vfio device, so.. we will too
	int groupid; 
	int groupfd;

	groupid = atoi(argv[1]); // we don't know if zero is the actual groupid number or atoi failing
	if (groupid == 0) {
		fprintf(stderr, "warning, groupid is zero, which means that you could have provided an invalid groupid\n");
	}

	snprintf(gid_path, sizeof(gid_path), "/dev/nvidia-vgpu%d", groupid);
	printf("opening groupid: %s\n", gid_path);

	groupfd = open(gid_path, O_RDWR);
	if (groupfd < 0) {
		perror("open");
		return 1;
	}

	off_t CONSOLE_OFFSET = 0x10000000000;
	pixel* surface; // todo call IOCTL to get size
	surface = (pixel*) mmap(0, 1920 * 1080 * 4, PROT_READ, MAP_PRIVATE, groupfd, CONSOLE_OFFSET);
	// I know it complains, but this is literally what it does
	if (surface < 0) {
		perror("mmap");
		return 1;
	}

	for (int i = 1080; i < 2160; ++i) { // each row
		for (int j = 1920; j < 3840; ++j) { // each pix per roww
			// printf("writing pixel (x, y) -> (%d, %d)\n", j, i);
			pixel pix = surface[(i * 1920) + j];
			bmp_pixel_init(&img.img_pixels[i][j], pix.r, pix.g, pix.b);
		}
	}

	bmp_img_write(&img, "test.bmp");
	bmp_img_free(&img);

	printf("image captured successfully and written to test.bmp\n");

}
