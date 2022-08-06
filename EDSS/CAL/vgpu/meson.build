project('edss', 'c',
  version : '0.1')


avutil = dependency('libavutil')
strmap = '../../vendor/strmap/strmap.c'

library('CalPluginVgpu',
           'calPluginVgpu.c',
           strmap,
           dependencies: [avutil])
