<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import {
  Download,
  Upload,
  RotateCcw,
  FileText,
  Globe,
  BookOpen,
  Sparkles,
  CloudUpload,
  CloudDownload,
  PlugZap,
  Loader2,
  Package,
} from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Dialog } from '@/components/ui/dialog'
import {
  DevOnly,
  SettingGroup,
  SettingRow,
  SettingSelect,
  SettingSwitch,
  SettingInput,
  ApiKeyInput,
} from '../components'
import type { AppSettings, RemoteBackupItem, WebDavStatus } from '../types'
import { useSettings } from '../stores/settings'
import {
  invokeBackupToWebDav,
  invokeExportLogs,
  invokeExportSettingsSnapshot,
  invokeImportSettingsSnapshot,
  invokeListWebDavBackups,
  invokeOpenUrl,
  invokeRestoreFromWebDav,
  invokeTestWebDavConnection,
  isTauriReady,
} from '@/lib/tauri'
import { useDevMode } from '../composables/useDevMode'
import { toast } from '@/lib/toast'
import { cn } from '@/lib/utils'
import { t } from '@/i18n'

const HOMEPAGE_URL = 'https://github.com/XuDaojie/shizi-translator'
const DEFAULT_REMOTE_DIR = '/shizi/backups/'

const props = defineProps<{
  state: AppSettings
}>()

const { reset, syncFromBackend } = useSettings()
const isDev = useDevMode()

const logLevelOptions = computed(() => [
  { label: 'Error', value: 'error' },
  { label: 'Warn', value: 'warn' },
  { label: 'Info', value: 'info' },
  { label: 'Debug', value: 'debug' },
])

const resetOpen = ref(false)
const restoreOpen = ref(false)
const importOpen = ref(false)
const importFileName = ref('')
const importPayload = ref<string | null>(null)
const fileInputRef = ref<HTMLInputElement | null>(null)

const testing = ref(false)
const backingUp = ref(false)
const restoring = ref(false)
const listingBackups = ref(false)
const remoteBackups = ref<RemoteBackupItem[]>([])
const selectedBackupId = ref('')
const exporting = ref(false)
const localExporting = ref(false)

const appVersion = ref('…')

const backup = computed(() => props.state.advanced.backup)
const webdav = computed(() => props.state.advanced.backup.webdav)

const connectionArgs = computed(() => ({
  url: webdav.value.url,
  username: webdav.value.username,
  password: webdav.value.password,
  remotePath: webdav.value.remotePath || DEFAULT_REMOTE_DIR,
}))

const canUseWebDav = computed(() => {
  const w = webdav.value
  return Boolean(w.url.trim() && w.username.trim() && w.password.trim() && w.remotePath.trim())
})

const isConnected = computed(() => webdav.value.status === 'connected')

const statusLabel = computed(() => {
  switch (webdav.value.status as WebDavStatus) {
    case 'connecting':
      return t('settings.backup.status.connecting')
    case 'connected':
      return t('settings.backup.status.connected')
    case 'error':
      return t('settings.backup.status.error')
    default:
      return t('settings.backup.status.idle')
  }
})

const statusVariant = computed(() => {
  switch (webdav.value.status as WebDavStatus) {
    case 'connected':
      return 'success' as const
    case 'error':
      return 'destructive' as const
    case 'connecting':
      return 'warning' as const
    default:
      return 'secondary' as const
  }
})

const formatTime = (iso: string): string => {
  if (!iso) return '—'
  try {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return '—'
    return d.toLocaleString(undefined, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    })
  } catch {
    return '—'
  }
}

const normalizeRemoteDir = (raw: string): string => {
  let p = raw.trim() || DEFAULT_REMOTE_DIR
  if (/\.(json|zip)$/i.test(p)) {
    const i = p.lastIndexOf('/')
    p = i > 0 ? p.slice(0, i + 1) : DEFAULT_REMOTE_DIR
  }
  if (!p.startsWith('/')) p = `/${p}`
  if (!p.endsWith('/')) p = `${p}/`
  return p
}

const remoteDir = computed(() => normalizeRemoteDir(webdav.value.remotePath))

/** 改凭证后作废「已连接」。 */
watch(
  () => [webdav.value.url, webdav.value.username, webdav.value.password] as const,
  () => {
    const w = webdav.value
    if (w.status === 'connected' || w.status === 'error') {
      w.status = 'idle'
      w.lastError = ''
    }
  },
)

onMounted(async () => {
  try {
    const tauri = (window as unknown as {
      __TAURI__?: { app?: { getVersion?: () => Promise<string> } }
    }).__TAURI__
    const version = await tauri?.app?.getVersion?.()
    if (version) appVersion.value = version
  } catch {
    // 非 Tauri 环境
  }
})

async function openHomepage() {
  try {
    await invokeOpenUrl(HOMEPAGE_URL)
  } catch (e) {
    toast.error(String(e))
  }
}

async function handleExportLogs() {
  if (exporting.value) return
  exporting.value = true
  try {
    const path = await invokeExportLogs()
    toast.success(t('settings.toast.logsExported'), path)
  } catch (e) {
    const msg = String(e)
    if (!msg.includes('取消')) {
      toast.error(t('settings.toast.exportFailed'), msg)
    }
  } finally {
    exporting.value = false
  }
}

const onTestConnection = async (): Promise<void> => {
  if (!canUseWebDav.value || testing.value) return
  if (!isTauriReady()) {
    toast.error(t('settings.backup.toast.needTauri'))
    return
  }
  const w = webdav.value
  testing.value = true
  w.status = 'connecting'
  w.lastError = ''
  try {
    w.remotePath = normalizeRemoteDir(w.remotePath)
    const result = await invokeTestWebDavConnection(connectionArgs.value)
    w.status = 'connected'
    w.lastTestedAt = result.lastTestedAt
    w.remotePath = result.remotePath || w.remotePath
    toast.success(t('settings.backup.toast.testOk'))
  } catch (e) {
    w.status = 'error'
    w.lastError = String(e)
    toast.error(t('settings.backup.toast.testFail'), w.lastError)
  } finally {
    testing.value = false
  }
}

const onBackupNow = async (): Promise<void> => {
  if (!isConnected.value || backingUp.value) return
  if (!isTauriReady()) {
    toast.error(t('settings.backup.toast.needTauri'))
    return
  }
  backingUp.value = true
  try {
    webdav.value.remotePath = normalizeRemoteDir(webdav.value.remotePath)
    const result = await invokeBackupToWebDav(connectionArgs.value)
    webdav.value.lastBackupAt = result.lastBackupAt
    const bits: string[] = []
    if (backup.value.includeHistory) bits.push(t('settings.backup.tag.history'))
    if (backup.value.includeApiKeys) bits.push(t('settings.backup.tag.apiKey'))
    toast.success(
      t('settings.backup.toast.backupOk'),
      `${result.remotePath}${bits.length ? `（${bits.join(' · ')}）` : ''}`,
    )
  } catch (e) {
    toast.error(t('settings.backup.toast.backupFail'), String(e))
  } finally {
    backingUp.value = false
  }
}

const loadRemoteBackups = async (): Promise<void> => {
  listingBackups.value = true
  remoteBackups.value = []
  selectedBackupId.value = ''
  try {
    const list = await invokeListWebDavBackups(connectionArgs.value)
    remoteBackups.value = list.map((item) => ({
      id: item.id,
      name: item.name,
      path: item.path,
      createdAt: item.createdAt,
      sizeLabel: item.sizeLabel,
      includeHistory: item.includeHistory,
      includeApiKeys: item.includeApiKeys,
    }))
    selectedBackupId.value = remoteBackups.value[0]?.id ?? ''
  } catch (e) {
    toast.error(t('settings.backup.toast.listFail'), String(e))
  } finally {
    listingBackups.value = false
  }
}

watch(restoreOpen, (open) => {
  if (open) void loadRemoteBackups()
  else {
    remoteBackups.value = []
    selectedBackupId.value = ''
  }
})

const selectedBackup = computed(
  () => remoteBackups.value.find((b) => b.id === selectedBackupId.value) ?? null,
)

const onConfirmRestore = async (): Promise<void> => {
  if (!isConnected.value || restoring.value || !selectedBackup.value) return
  restoring.value = true
  try {
    await invokeRestoreFromWebDav(connectionArgs.value, selectedBackup.value.path)
    await syncFromBackend()
    toast.success(t('settings.backup.toast.restoreOk'), selectedBackup.value.name)
    restoreOpen.value = false
  } catch (e) {
    toast.error(t('settings.backup.toast.restoreFail'), String(e))
  } finally {
    restoring.value = false
  }
}

const downloadText = (text: string, filename: string): void => {
  const blob = new Blob([text], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

const onExportLocal = async (): Promise<void> => {
  if (localExporting.value) return
  localExporting.value = true
  try {
    if (!isTauriReady()) {
      toast.error(t('settings.backup.toast.needTauri'))
      return
    }
    const json = await invokeExportSettingsSnapshot()
    const stamp = new Date().toISOString().slice(0, 10)
    downloadText(json, `shizi-settings-${stamp}.json`)
    toast.success(t('settings.backup.toast.exportOk'))
  } catch (e) {
    toast.error(t('settings.toast.exportFailed'), String(e))
  } finally {
    localExporting.value = false
  }
}

const onPickImport = (): void => {
  fileInputRef.value?.click()
}

const onImportFile = async (ev: Event): Promise<void> => {
  const input = ev.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  try {
    const text = await file.text()
    JSON.parse(text)
    importFileName.value = file.name
    importPayload.value = text
    importOpen.value = true
  } catch {
    toast.error(t('settings.backup.toast.parseFail'))
  }
}

const onConfirmImport = async (): Promise<void> => {
  if (!importPayload.value) return
  try {
    if (!isTauriReady()) {
      toast.error(t('settings.backup.toast.needTauri'))
      return
    }
    await invokeImportSettingsSnapshot(importPayload.value)
    await syncFromBackend()
    toast.success(t('settings.backup.toast.importOk'), importFileName.value || undefined)
    importOpen.value = false
    importPayload.value = null
  } catch (e) {
    toast.error(t('settings.backup.toast.importFail'), String(e))
  }
}

// 自动备份：在 settings store 保存成功后 scheduleAutoBackupIfNeeded（约 30s）
</script>

<template>
  <!-- 主路径：WebDAV；顺序：连接 → 含历史 → 含 Key → 手动同步 → 自动备份 -->
  <SettingGroup>
    <template #header>
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <h3 class="text-[13px] font-semibold text-foreground">
            {{ t('settings.backup.groupTitle') }}
          </h3>
          <p class="mt-1 text-xs text-muted-foreground leading-snug">
            {{ t('settings.backup.groupDesc') }}
          </p>
        </div>
        <Badge :variant="statusVariant" class="shrink-0">
          <span
            class="inline-block h-1.5 w-1.5 rounded-full"
            :class="{
              'bg-emerald-500': webdav.status === 'connected',
              'bg-destructive': webdav.status === 'error',
              'bg-amber-500 animate-pulse': webdav.status === 'connecting',
              'bg-muted-foreground': webdav.status === 'idle',
            }"
          />
          {{ statusLabel }}
        </Badge>
      </div>
    </template>

    <SettingRow
      :title="t('settings.backup.connection')"
      :description="t('settings.backup.connectionDesc')"
      vertical
    >
      <div class="grid grid-cols-1 gap-2.5 sm:grid-cols-2">
        <div class="sm:col-span-2 space-y-1">
          <label class="text-[11px] text-muted-foreground" for="webdav-url">
            {{ t('settings.backup.url') }}
          </label>
          <SettingInput
            id="webdav-url"
            v-model="state.advanced.backup.webdav.url"
            type="url"
            placeholder="https://dav.jianguoyun.com/dav/"
            class="font-mono"
          />
        </div>
        <div class="space-y-1">
          <label class="text-[11px] text-muted-foreground" for="webdav-user">
            {{ t('settings.backup.username') }}
          </label>
          <SettingInput
            id="webdav-user"
            v-model="state.advanced.backup.webdav.username"
            :placeholder="t('settings.backup.usernamePlaceholder')"
          />
        </div>
        <div class="space-y-1">
          <label class="text-[11px] text-muted-foreground">
            {{ t('settings.backup.password') }}
          </label>
          <ApiKeyInput
            v-model="state.advanced.backup.webdav.password"
            :placeholder="t('settings.backup.passwordPlaceholder')"
            :allow-validate="false"
          />
        </div>
        <div class="sm:col-span-2 space-y-1">
          <label class="text-[11px] text-muted-foreground" for="webdav-path">
            {{ t('settings.backup.remotePath') }}
          </label>
          <SettingInput
            id="webdav-path"
            v-model="state.advanced.backup.webdav.remotePath"
            placeholder="/shizi/backups/"
            class="font-mono"
          />
          <p class="text-[11px] text-muted-foreground">
            {{ t('settings.backup.remotePathHelp') }}
            <code class="rounded bg-muted px-1 py-0.5 font-mono text-[10px]">
              shizi-backup-20260805-153012.zip
            </code>
          </p>
        </div>
      </div>
      <div class="mt-2.5 flex flex-wrap items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          :disabled="!canUseWebDav || testing"
          @click="onTestConnection"
        >
          <Loader2 v-if="testing" class="h-3.5 w-3.5 animate-spin" />
          <PlugZap v-else class="h-3.5 w-3.5" />
          {{ t('settings.backup.testConnection') }}
        </Button>
        <span v-if="webdav.lastTestedAt" class="text-[11px] text-muted-foreground">
          {{ t('settings.backup.lastTested') }}：{{ formatTime(webdav.lastTestedAt) }}
        </span>
        <span v-else-if="webdav.lastError" class="text-[11px] text-destructive">
          {{ webdav.lastError }}
        </span>
      </div>
    </SettingRow>

    <SettingRow
      :title="t('settings.backup.includeHistory')"
      :description="t('settings.backup.includeHistoryDesc')"
    >
      <SettingSwitch
        v-model="state.advanced.backup.includeHistory"
        :aria-label="t('settings.backup.includeHistory')"
      />
    </SettingRow>

    <SettingRow
      :title="t('settings.backup.includeApiKeys')"
      :description="t('settings.backup.includeApiKeysDesc')"
    >
      <SettingSwitch
        v-model="state.advanced.backup.includeApiKeys"
        :aria-label="t('settings.backup.includeApiKeys')"
      />
    </SettingRow>

    <!-- 手动同步：选项之后的最终操作 -->
    <div
      class="flex min-h-[2.375rem] items-center justify-between gap-3 px-2.5 py-2 transition-colors duration-150 hover:bg-muted/40"
    >
      <div class="min-w-0 flex-1">
        <div class="text-[13px] font-medium text-foreground">
          {{ t('settings.backup.manualSync') }}
        </div>
        <p class="mt-0.5 text-[11px] leading-snug text-muted-foreground">
          {{ t('settings.backup.manualSyncDesc') }}
        </p>
        <p class="mt-1.5 text-[11px] text-muted-foreground">
          {{ t('settings.backup.lastBackup') }}：
          <span class="font-medium text-foreground">{{ formatTime(webdav.lastBackupAt) }}</span>
        </p>
      </div>
      <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
        <Button
          size="sm"
          :disabled="!isConnected || backingUp"
          @click="onBackupNow"
        >
          <Loader2 v-if="backingUp" class="h-3.5 w-3.5 animate-spin" />
          <CloudUpload v-else class="h-3.5 w-3.5" />
          {{ t('settings.backup.backupNow') }}
        </Button>
        <Dialog
          v-model:open="restoreOpen"
          :title="t('settings.backup.restoreTitle')"
          :description="t('settings.backup.restoreDescription')"
          width="480px"
        >
          <template #trigger>
            <Button
              variant="outline"
              size="sm"
              :disabled="!isConnected || restoring"
            >
              <CloudDownload class="h-3.5 w-3.5" />
              {{ t('settings.backup.restoreFromCloud') }}
            </Button>
          </template>

          <div class="space-y-3">
            <p class="text-[11px] text-muted-foreground">
              {{ t('settings.backup.directory') }}
              <code class="rounded bg-muted px-1 py-0.5 font-mono text-[10px]">{{ remoteDir }}</code>
            </p>

            <div
              v-if="listingBackups"
              class="flex items-center justify-center gap-2 rounded-md border border-border py-10 text-sm text-muted-foreground"
            >
              <Loader2 class="h-4 w-4 animate-spin" />
              {{ t('settings.backup.listing') }}
            </div>

            <div
              v-else-if="remoteBackups.length === 0"
              class="rounded-md border border-dashed border-border py-10 text-center text-sm text-muted-foreground"
            >
              {{ t('settings.backup.emptyList') }}
            </div>

            <ul
              v-else
              class="max-h-[280px] space-y-1.5 overflow-y-auto scrollbar-thin pr-0.5"
              role="listbox"
              :aria-label="t('settings.backup.listAria')"
            >
              <li
                v-for="item in remoteBackups"
                :key="item.id"
                role="option"
                :aria-selected="selectedBackupId === item.id"
                :class="cn(
                  'flex cursor-pointer items-start gap-2.5 rounded-md border px-2.5 py-2 transition-colors',
                  selectedBackupId === item.id
                    ? 'border-primary bg-primary/5'
                    : 'border-border hover:bg-muted/50',
                )"
                @click="selectedBackupId = item.id"
              >
                <span
                  class="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border"
                  :class="
                    selectedBackupId === item.id
                      ? 'border-primary'
                      : 'border-muted-foreground/40'
                  "
                >
                  <span
                    v-if="selectedBackupId === item.id"
                    class="h-2 w-2 rounded-full bg-primary"
                  />
                </span>
                <Package class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
                <div class="min-w-0 flex-1">
                  <p class="truncate font-mono text-[12px] font-medium text-foreground">
                    {{ item.name }}
                  </p>
                  <p class="mt-0.5 text-[11px] text-muted-foreground">
                    {{ formatTime(item.createdAt) }}
                    <span class="mx-1 text-border">·</span>
                    {{ item.sizeLabel }}
                  </p>
                </div>
              </li>
            </ul>

            <div class="flex justify-end gap-2 pt-1">
              <Button variant="ghost" @click="restoreOpen = false">{{ t('common.cancel') }}</Button>
              <Button
                variant="destructive"
                :disabled="restoring || listingBackups || !selectedBackup"
                @click="onConfirmRestore"
              >
                <Loader2 v-if="restoring" class="h-3.5 w-3.5 animate-spin" />
                {{ t('settings.backup.confirmRestore') }}
              </Button>
            </div>
          </div>
        </Dialog>
      </div>
    </div>

    <SettingRow
      :title="t('settings.backup.autoSync')"
      :description="t('settings.backup.autoSyncDesc')"
    >
      <SettingSwitch
        v-model="state.advanced.backup.autoSync"
        :aria-label="t('settings.backup.autoSync')"
      />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    :title="t('settings.group.data')"
    :description="t('settings.description.data')"
  >
    <SettingRow
      :title="t('settings.field.exportConfig')"
      :description="t('settings.description.exportConfig')"
    >
      <Button variant="outline" size="sm" :disabled="localExporting" @click="onExportLocal">
        <Upload class="h-3.5 w-3.5" />
        {{ t('common.export') }}
      </Button>
    </SettingRow>
    <SettingRow
      :title="t('settings.field.importConfig')"
      :description="t('settings.description.importConfig')"
    >
      <Button variant="outline" size="sm" @click="onPickImport">
        <Download class="h-3.5 w-3.5" />
        {{ t('common.import') }}
      </Button>
      <input
        ref="fileInputRef"
        type="file"
        accept="application/json,.json"
        class="sr-only"
        @change="onImportFile"
      />
    </SettingRow>

    <Dialog
      v-model:open="importOpen"
      :title="t('settings.backup.importTitle')"
      :description="t('settings.backup.importDescription', { name: importFileName || '…' })"
      width="420px"
    >
      <div class="flex justify-end gap-2">
        <Button
          variant="ghost"
          @click="
            () => {
              importOpen = false
              importPayload = null
            }
          "
        >
          {{ t('common.cancel') }}
        </Button>
        <Button variant="destructive" @click="onConfirmImport">
          {{ t('settings.backup.confirmImport') }}
        </Button>
      </div>
    </Dialog>
  </SettingGroup>

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

  <SettingGroup :title="t('settings.group.about')">
    <SettingRow :title="t('settings.field.version')" :description="t('settings.description.version')">
      <span class="text-sm text-muted-foreground font-mono">
        v{{ appVersion }}{{ isDev ? ' · dev' : '' }}
      </span>
    </SettingRow>
    <SettingRow :title="t('settings.field.homepage')" :description="t('settings.description.homepage')">
      <Button variant="ghost" size="sm" @click="openHomepage">
        <Globe class="h-3.5 w-3.5" />
        {{ t('common.visit') }}
      </Button>
    </SettingRow>
    <DevOnly>
      <SettingRow :title="t('settings.field.changelog')" :description="t('settings.description.changelog')" status="wip">
        <Button variant="ghost" size="sm">
          <FileText class="h-3.5 w-3.5" />
          {{ t('common.open') }}
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
    </DevOnly>
  </SettingGroup>
</template>
