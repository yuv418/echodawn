project('edss', 'c',
  version : '0.1')


avutil = dependency('libavutil')
strmap = '../../vendor/strmap/strmap.c'
edss_log = '../../src/edssLog.c'

library('CalPluginVgpu',
           'calPluginVgpu.c',
           edss_log,
           strmap,
           dependencies: [avutil])
