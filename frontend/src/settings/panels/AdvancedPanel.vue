<script setup lang="ts">
import { ref } from 'vue'
import { Download, Upload, RotateCcw, FileText, Globe, BookOpen, Sparkles } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Dialog } from '@/components/ui/dialog'
import { SettingGroup, SettingRow, SettingSelect, SettingSwitch } from '../components'
import type { AppSettings } from '../types'
import { useSettings } from '../stores/settings'

defineProps<{
  state: AppSettings
}>()

const { reset } = useSettings()

const logLevelOptions = [
  { label: '错误', value: 'error' },
  { label: '警告', value: 'warn' },
  { label: '信息', value: 'info' },
  { label: '调试', value: 'debug' },
]

const resetOpen = ref(false)
const exportOpen = ref(false)
const importOpen = ref(false)

const appVersion = '0.1.0'
const buildChannel = 'dev'
</script>

<template>
  <SettingGroup title="日志" description="本地日志等级与导出能力,帮助排查异常。">
    <SettingRow
      title="日志等级"
      description="调试等级会产生大量日志,默认使用信息等级即可。"
    >
      <SettingSelect v-model="state.advanced.logLevel" :options="logLevelOptions" />
    </SettingRow>
    <SettingRow
      title="导出日志"
      description="将最近 7 天的日志打包到一个 zip 文件,便于提交反馈。"
    >
      <Button variant="outline" size="sm">
        <Download class="h-3.5 w-3.5" />
        导出
      </Button>
    </SettingRow>
  </SettingGroup>

  <SettingGroup title="实验性功能" description="尚在迭代中的能力,默认关闭,可能在未来版本移除或调整。">

  <SettingGroup title="隐私" description="匿名使用统计帮助改进产品，不包含翻译内容与 API Key。">
    <SettingRow
      title="收集匿名使用统计"
      description="重启后生效。"
    >
      <SettingSwitch v-model="state.advanced.collectUsage" aria-label="收集匿名使用统计" />
    </SettingRow>
  </SettingGroup>
    <SettingRow
      title="启用取词翻译增强"
      description="对 PDF、终端等特殊窗口的取词提供额外兼容。"
    >
      <SettingSwitch v-model="state.advanced.betaLookup" aria-label="取词增强" />
    </SettingRow>
    <SettingRow
      title="启用语音输入"
      description="在翻译面板中按住空格录入语音并翻译。"
      status="wip"
    >
      <SettingSwitch v-model="state.advanced.betaVoice" aria-label="语音输入" />
    </SettingRow>
  </SettingGroup>

  <SettingGroup title="数据" description="配置导入导出与重置。">
    <SettingRow
      title="导出配置"
      description="将设置项(不含 API Key)导出为 JSON 文件,便于迁移。"
    >
      <Dialog
        v-model:open="exportOpen"
        title="导出配置"
        description="API Key 不会包含在导出文件中,以避免泄露。"
        width="420px"
      >
        <template #trigger>
          <Button variant="outline" size="sm">
            <Upload class="h-3.5 w-3.5" />
            导出
          </Button>
        </template>
        <p class="text-sm text-muted-foreground">
          导出文件名为 <code class="rounded bg-muted px-1.5 py-0.5">settings.json</code>。
        </p>
        <div class="flex justify-end">
          <Button @click="exportOpen = false">知道了</Button>
        </div>
      </Dialog>
    </SettingRow>
    <SettingRow
      title="导入配置"
      description="从 JSON 文件恢复设置项,API Key 不会被覆盖。"
    >
      <Dialog
        v-model:open="importOpen"
        title="导入配置"
        description="选择要导入的配置文件。"
        width="420px"
      >
        <template #trigger>
          <Button variant="outline" size="sm">
            <Download class="h-3.5 w-3.5" />
            导入
          </Button>
        </template>
        <p class="text-sm text-muted-foreground">占位:实际实现会打开文件选择对话框并解析 JSON。</p>
        <div class="flex justify-end">
          <Button @click="importOpen = false">关闭</Button>
        </div>
      </Dialog>
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    title="重置"
    description="将所有设置恢复为默认,操作不可撤销。"
  >
    <SettingRow
      title="重置全部设置"
      description="清空所有自定义项,包括已配置的 API Key。"
    >
      <Dialog
        v-model:open="resetOpen"
        title="重置全部设置?"
        description="此操作会清空你配置的所有翻译服务、快捷键与个性化选项,且无法恢复。"
        width="420px"
      >
        <template #trigger>
          <Button variant="destructive" size="sm">
            <RotateCcw class="h-3.5 w-3.5" />
            重置
          </Button>
        </template>
        <div class="flex justify-end gap-2">
          <Button variant="ghost" @click="resetOpen = false">取消</Button>
          <Button
            variant="destructive"
            @click="
              () => {
                reset()
                resetOpen = false
              }
            "
          >
            确认重置
          </Button>
        </div>
      </Dialog>
    </SettingRow>
  </SettingGroup>

  <SettingGroup title="关于">
    <SettingRow title="版本" description="查看本应用的当前版本与构建信息。">
      <span class="text-sm text-muted-foreground font-mono">
        v{{ appVersion }} · {{ buildChannel }}
      </span>
    </SettingRow>
    <SettingRow title="查看更新日志" description="了解每个版本新增的能力与修复。">
      <Button variant="ghost" size="sm">
        <FileText class="h-3.5 w-3.5" />
        打开
      </Button>
    </SettingRow>
    <SettingRow title="项目主页" description="在 Globe 查看源码、提交反馈。">
      <Button variant="ghost" size="sm">
        <Globe class="h-3.5 w-3.5" />
        访问
      </Button>
    </SettingRow>
    <SettingRow title="使用文档" description="入门指引、快捷键清单、常见问题。">
      <Button variant="ghost" size="sm">
        <BookOpen class="h-3.5 w-3.5" />
        查看
      </Button>
    </SettingRow>
    <SettingRow title="推荐给朋友" description="向朋友推荐本应用,共同完善产品。">
      <Button variant="ghost" size="sm">
        <Sparkles class="h-3.5 w-3.5" />
        分享
      </Button>
    </SettingRow>
  </SettingGroup>
</template>
