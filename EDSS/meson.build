project('edss', 'c',
  version : '0.1')


avutil = dependency('libavutil')
avcodec = dependency('libavcodec')
swscale = dependency('libswscale')
avformat = dependency('libavformat')
ck = dependency('ck')

strmap = 'vendor/strmap/strmap.c'

library('edss',
           'src/edssLog.c',
           'src/edssInterface.c',
           'src/edssCapture.c',
           strmap,
           dependencies: [avutil, avcodec, swscale, avformat, ck])
