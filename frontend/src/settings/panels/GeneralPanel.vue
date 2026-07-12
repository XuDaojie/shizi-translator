<script setup lang="ts">
import {
  SettingGroup,
  SettingRow,
  SettingSelect,
  SettingSwitch,
  DevOnly,
} from '../components'
import type { AppSettings } from '../types'
import { computed } from 'vue'
import { FolderOpen, RefreshCw } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { t } from '@/i18n'
import { toast } from '@/lib/toast'
import { useSettings } from '../stores/settings'

defineProps<{
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

const closeActionOptions = computed(() => [
  { label: t('settings.option.minimize'), value: 'minimize' },
  { label: t('settings.option.quit'), value: 'quit' },
])

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
    <SettingRow
      :title="t('settings.field.startMinimized')"
      :description="t('settings.description.startMinimized')"
    >
      <SettingSwitch v-model="state.general.startMinimized" :aria-label="t('settings.field.startMinimized')" />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.showTrayIcon')"
      :description="t('settings.description.showTrayIcon')"
    >
      <SettingSwitch v-model="state.general.showTrayIcon" :aria-label="t('settings.field.showTrayIcon')" />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.closeBehavior')"
      :description="t('settings.description.closeBehavior')"
    >
      <SettingSelect
        v-model="state.general.closeAction"
        :options="closeActionOptions"
      />
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
    <DevOnly>
      <SettingRow
        :title="t('settings.field.autoCheckUpdate')"
        :description="t('settings.description.autoCheckUpdate')"
        status="wip"
      >
        <SettingSwitch v-model="state.general.autoCheckUpdate" :aria-label="t('settings.field.autoCheckUpdate')" />
      </SettingRow>
    </DevOnly>
  </SettingGroup>
</template>
