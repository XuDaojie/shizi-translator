<script setup lang="ts">
import {
  SettingGroup,
  SettingRow,
  SettingSelect,
  SettingSwitch,
  DevOnly,
} from '../components'
import type { AppSettings } from '../types'
import { computed, onMounted, ref } from 'vue'
import { FolderOpen, RefreshCw } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Dialog } from '@/components/ui/dialog'
import { t } from '@/i18n'
import { toast } from '@/lib/toast'
import {
  invokeCheckForUpdate,
  invokeOpenUrl,
  type CheckUpdateResult,
} from '@/lib/tauri'
import { useSettings } from '../stores/settings'

const props = defineProps<{
  state: AppSettings
}>()

const { interfaceLanguages, interfaceLanguageErrors, interfaceLanguagesRefreshing, refreshInterfaceLanguages, openLanguagePackDirectory, setInterfaceLanguage } = useSettings()

const themeOptions = computed(() => [
  { label: t('settings.option.light'), value: 'light' },
  { label: t('settings.option.dark'), value: 'dark' },
  { label: t('settings.option.system'), value: 'system' },
])
const languageOptions = computed(() => [
  { label: t('language.auto'), value: 'auto' },
  ...interfaceLanguages.value.map(({ locale, name }) => ({ label: name, value: locale })),
])

const updateChannelOptions = computed(() => [
  { label: t('settings.option.stable'), value: 'stable' },
  { label: 'Beta', value: 'beta' },
])

const checking = ref(false)
const updateDialogOpen = ref(false)
const pendingUpdate = ref<CheckUpdateResult | null>(null)
const appVersion = ref('…')

const updateDialogDescription = computed(() => {
  const result = pendingUpdate.value
  if (!result) return ''
  const prerelease = result.isPrerelease
    ? t('settings.dialog.updatePrereleaseSuffix')
    : ''
  return t('settings.dialog.updateDescription', {
    latest: result.latestVersion ?? '',
    current: result.currentVersion,
    prerelease,
  })
})

onMounted(async () => {
  try {
    const tauri = (window as unknown as {
      __TAURI__?: { app?: { getVersion?: () => Promise<string> } }
    }).__TAURI__
    const v = await tauri?.app?.getVersion?.()
    if (v) appVersion.value = v
  } catch { /* vite only */ }
})

async function handleCheckUpdate() {
  if (checking.value) return
  checking.value = true
  try {
    const result = await invokeCheckForUpdate(props.state.general.updateChannel)
    if (result.status === 'up_to_date') {
      // Nightly 等场景后端会带 message，优先展示说明，避免仅显示「已是最新」造成误解
      if (result.message) {
        toast.success(result.message)
      } else {
        toast.success(t('settings.toast.upToDate', { version: result.currentVersion }))
      }
    } else if (result.status === 'update_available') {
      pendingUpdate.value = result
      updateDialogOpen.value = true
    } else {
      toast.error(
        t('settings.toast.checkUpdateFailed'),
        result.message ?? '',
      )
    }
  } catch (e) {
    toast.error(t('settings.toast.checkUpdateFailed'), String(e))
  } finally {
    checking.value = false
  }
}

async function goDownload() {
  const url = pendingUpdate.value?.releaseUrl
  updateDialogOpen.value = false
  if (!url) return
  try {
    await invokeOpenUrl(url)
  } catch (e) {
    toast.error(t('settings.toast.openUrlFailed'), String(e))
  }
}

const refreshLanguages = async () => {
  try { await refreshInterfaceLanguages() } catch (error) { toast.error(t('settings.toast.refreshFailed'), String(error)) }
}
const openDirectory = async () => {
  try { await openLanguagePackDirectory() } catch (error) { toast.error(t('settings.toast.openLanguageDirectoryFailed'), String(error)) }
}
</script>

<template>
  <SettingGroup
    :title="t('settings.group.startup')"
    :description="t('settings.description.startup')"
  >
    <SettingRow
      :title="t('settings.field.launchAtLogin')"
      :description="t('settings.description.launchAtLogin')"
    >
      <SettingSwitch v-model="state.general.launchAtLogin" :aria-label="t('settings.field.launchAtLogin')" />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    :title="t('settings.group.window')"
    :description="t('settings.description.windowPolicy')"
  >
    <SettingRow
      :title="t('settings.field.popupPrecreate')"
      :description="t('settings.description.popupPrecreate')"
    >
      <SettingSwitch v-model="state.general.popupPrecreate" :aria-label="t('settings.field.popupPrecreate')" />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.overlayPrecreate')"
      :description="t('settings.description.overlayPrecreate')"
    >
      <SettingSwitch v-model="state.general.overlayPrecreate" :aria-label="t('settings.field.overlayPrecreate')" />
    </SettingRow>
  </SettingGroup>

  <SettingGroup :title="t('settings.group.appearance')" :description="t('settings.description.appearance')">
    <DevOnly>
      <SettingRow
        :title="t('settings.field.theme')"
        :description="t('settings.description.theme')"
        status="wip"
      >
        <SettingSelect v-model="state.general.theme" :options="themeOptions" />
      </SettingRow>
    </DevOnly>
    <SettingRow :title="t('settings.field.interfaceLanguage')" :description="t('settings.description.interfaceLanguage')">
      <div class="flex items-center gap-1">
        <SettingSelect :model-value="state.general.language" :options="languageOptions" @update:model-value="setInterfaceLanguage" />
        <DevOnly>
          <div class="flex gap-1">
            <Button variant="ghost" size="icon" :title="t('settings.button.openLanguageDirectory')" :aria-label="t('settings.button.openLanguageDirectory')" @click="openDirectory"><FolderOpen class="h-4 w-4" /></Button>
            <Button variant="ghost" size="icon" :disabled="interfaceLanguagesRefreshing" :title="t('settings.button.refreshLanguages')" :aria-label="t('settings.button.refreshLanguages')" @click="refreshLanguages"><RefreshCw :class="['h-4 w-4', interfaceLanguagesRefreshing && 'animate-spin']" /></Button>
          </div>
        </DevOnly>
      </div>
    </SettingRow>
    <DevOnly>
      <p v-for="error in interfaceLanguageErrors" :key="`${error.file}:${error.message}`" class="text-xs text-destructive">{{ error.file }}: {{ error.message }}</p>
    </DevOnly>
  </SettingGroup>

  <SettingGroup :title="t('settings.group.update')" :description="t('settings.description.update')">
    <SettingRow
      :title="t('settings.field.updateChannel')"
      :description="t('settings.description.updateChannel')"
    >
      <SettingSelect v-model="state.general.updateChannel" :options="updateChannelOptions" />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.autoCheckUpdate')"
      :description="t('settings.description.autoCheckUpdate')"
    >
      <SettingSwitch v-model="state.general.autoCheckUpdate" :aria-label="t('settings.field.autoCheckUpdate')" />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.currentAppVersion')"
      :description="t('settings.description.version')"
    >
      <span class="text-sm text-muted-foreground font-mono">v{{ appVersion }}</span>
    </SettingRow>
    <SettingRow :title="t('settings.button.checkUpdate')" description="">
      <Button size="sm" :disabled="checking" @click="handleCheckUpdate">
        <RefreshCw :class="['h-3.5 w-3.5', checking && 'animate-spin']" />
        {{ t('settings.button.checkUpdate') }}
      </Button>
    </SettingRow>
    <Dialog
      v-model:open="updateDialogOpen"
      :title="t('settings.dialog.updateTitle')"
      :description="updateDialogDescription"
      width="420px"
    >
      <div class="flex justify-end gap-2">
        <Button variant="ghost" @click="updateDialogOpen = false">{{ t('settings.button.later') }}</Button>
        <Button @click="goDownload">{{ t('settings.button.goDownload') }}</Button>
      </div>
    </Dialog>
  </SettingGroup>
</template>
