<script setup lang="ts">
import { computed, ref } from 'vue'
import { Download, Upload, RotateCcw, FileText, Globe, BookOpen, Sparkles } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Dialog } from '@/components/ui/dialog'
import { DevOnly, SettingGroup, SettingRow, SettingSelect, SettingSwitch } from '../components'
import type { AppSettings } from '../types'
import { useSettings } from '../stores/settings'
import { exportSettings, importSettings, parseImportedSettings } from '../config-io'
import { invokeExportLogs } from '@/lib/tauri'
import { toast } from '@/lib/toast'
import { t } from '@/i18n'

const props = defineProps<{
  state: AppSettings
}>()

const { reset } = useSettings()

const logLevelOptions = computed(() => [
  { label: 'Error', value: 'error' },
  { label: 'Warn', value: 'warn' },
  { label: 'Info', value: 'info' },
  { label: 'Debug', value: 'debug' },
])

const resetOpen = ref(false)
const fileInput = ref<HTMLInputElement>()

const appVersion = '0.1.0'
const buildChannel = 'dev'

const exporting = ref(false)

async function handleExportLogs() {
  if (exporting.value) return
  exporting.value = true
  try {
    const path = await invokeExportLogs()
    toast.success(t('settings.toast.logsExported'), path)
  } catch (e) {
    const msg = String(e)
    if (msg.includes('取消')) {
      // 用户取消，不提示错误
    } else {
      toast.error(t('settings.toast.exportFailed'), msg)
    }
  } finally {
    exporting.value = false
  }
}

function handleExport() {
  const blob = new Blob([JSON.stringify(exportSettings(props.state), null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'settings.json'
  a.click()
  URL.revokeObjectURL(url)
}

function handleImport() {
  fileInput.value?.click()
}

async function onFileChange(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0]
  if (!file) return
  const text = await file.text()
  const incoming = parseImportedSettings(text)
  const merged = importSettings(props.state, incoming)
  Object.assign(props.state, merged)
}
</script>

<template>
  <SettingGroup :title="t('settings.group.logging')" :description="t('settings.description.logging')">
    <SettingRow
      :title="t('settings.field.logLevel')"
      :description="t('settings.description.logLevel')"
    >
      <SettingSelect v-model="state.advanced.logLevel" :options="logLevelOptions" />
    </SettingRow>
    <SettingRow
      :title="t('settings.button.exportLogs')"
      :description="t('settings.description.exportLogs')"
    >
      <Button variant="outline" size="sm" :disabled="exporting" @click="handleExportLogs">
        <Download class="h-3.5 w-3.5" />
        {{ t('common.export') }}
      </Button>
    </SettingRow>
  </SettingGroup>


  <DevOnly>
    <SettingGroup :title="t('settings.group.privacy')" :description="t('settings.description.privacy')">
      <SettingRow
        :title="t('settings.field.collectUsage')"
        :description="t('settings.description.restartRequired')"
        status="wip"
      >
        <SettingSwitch v-model="state.advanced.collectUsage" :aria-label="t('settings.field.collectUsage')" />
      </SettingRow>
    </SettingGroup>
  </DevOnly>

  <SettingGroup :title="t('settings.group.data')" :description="t('settings.description.data')">
    <SettingRow
      :title="t('settings.field.exportConfig')"
      :description="t('settings.description.exportConfig')"
    >
      <Button variant="outline" size="sm" @click="handleExport">
        <Upload class="h-3.5 w-3.5" />
        {{ t('common.export') }}
      </Button>
    </SettingRow>
    <SettingRow
      :title="t('settings.field.importConfig')"
      :description="t('settings.description.importConfig')"
    >
      <input ref="fileInput" type="file" accept=".json" hidden @change="onFileChange" />
      <Button variant="outline" size="sm" @click="handleImport">
        <Download class="h-3.5 w-3.5" />
        {{ t('common.import') }}
      </Button>
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    :title="t('settings.group.danger')"
    :description="t('settings.description.reset')"
  >
    <SettingRow
      :title="t('settings.field.resetAll')"
      :description="t('settings.description.resetAll')"
    >
      <Dialog
        v-model:open="resetOpen"
        :title="t('settings.dialog.resetTitle')"
        :description="t('settings.dialog.resetDescription')"
        width="420px"
      >
        <template #trigger>
          <Button variant="destructive" size="sm">
            <RotateCcw class="h-3.5 w-3.5" />
            {{ t('settings.button.reset') }}
          </Button>
        </template>
        <div class="flex justify-end gap-2">
          <Button variant="ghost" @click="resetOpen = false">{{ t('common.cancel') }}</Button>
          <Button
            variant="destructive"
            @click="
              () => {
                reset()
                resetOpen = false
              }
            "
          >
            {{ t('settings.button.confirmReset') }}
          </Button>
        </div>
      </Dialog>
    </SettingRow>
  </SettingGroup>

  <DevOnly>
    <SettingGroup :title="t('settings.group.about')">
      <SettingRow :title="t('settings.field.version')" :description="t('settings.description.version')" status="wip">
        <span class="text-sm text-muted-foreground font-mono">
          v{{ appVersion }} · {{ buildChannel }}
        </span>
      </SettingRow>
      <SettingRow :title="t('settings.field.changelog')" :description="t('settings.description.changelog')" status="wip">
        <Button variant="ghost" size="sm">
          <FileText class="h-3.5 w-3.5" />
          {{ t('common.open') }}
        </Button>
      </SettingRow>
      <SettingRow :title="t('settings.field.homepage')" :description="t('settings.description.homepage')" status="wip">
        <Button variant="ghost" size="sm">
          <Globe class="h-3.5 w-3.5" />
          {{ t('common.visit') }}
        </Button>
      </SettingRow>
      <SettingRow :title="t('settings.field.documentation')" :description="t('settings.description.documentation')" status="wip">
        <Button variant="ghost" size="sm">
          <BookOpen class="h-3.5 w-3.5" />
          {{ t('common.view') }}
        </Button>
      </SettingRow>
      <SettingRow :title="t('settings.field.recommend')" :description="t('settings.description.recommend')" status="wip">
        <Button variant="ghost" size="sm">
          <Sparkles class="h-3.5 w-3.5" />
          {{ t('common.share') }}
        </Button>
      </SettingRow>
    </SettingGroup>
  </DevOnly>
</template>
