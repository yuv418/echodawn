all:
	# gcc -Wall vgpu-catpure-test.c libbmp.c -o main
	gcc -Og -Wall -g vgpu-video-test.c -o main -lavutil -lavcodec -lswscale -lavformat
	gcc -Og -Wall -g vgpu-catpure-test.c libbmp.c -o capture
	gcc -Og -Wall -g vgpuVideo.c capture.c libbmp.c -o main_multithread -lavutil -lavcodec -lswscale -lavformat -lck

clean: 
	rm main test.bmp
