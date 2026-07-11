<script setup lang="ts">
import { computed } from 'vue'
import { SettingGroup, SettingRow, ShortcutRecorder } from '../components'
import type { AppSettings } from '../types'
import { t, type MessageKey } from '@/i18n'

const props = defineProps<{
  state: AppSettings
}>()

/** 全局快捷键：系统级注册，应用未聚焦时也生效。 */
const GLOBAL_IDS = new Set([
  'translate-selection',
  'translate-clipboard',
  'translate-screenshot',
])

/** 程序快捷键：仅应用窗口聚焦时由前端处理。 */
const APP_IDS = new Set(['open-settings'])

const globalBindings = computed(() =>
  props.state.shortcut.bindings.filter((b) => GLOBAL_IDS.has(b.id)),
)

const appBindings = computed(() =>
  props.state.shortcut.bindings.filter((b) => APP_IDS.has(b.id)),
)

const conflictedBindings = computed(() =>
  props.state.shortcut.bindings.filter((b) => b.error && GLOBAL_IDS.has(b.id)),
)
const bindingText = (id: string, kind: 'label' | 'description') =>
  t(`settings.shortcut.${id}.${kind}` as MessageKey)
</script>

<template>
  <SettingGroup
    :title="t('settings.group.globalShortcuts')"
    :description="t('settings.description.globalShortcuts')"
  >
    <SettingRow
      v-for="binding in globalBindings"
      :key="binding.id"
      :title="bindingText(binding.id, 'label')"
      :description="bindingText(binding.id, 'description')"
    >
      <ShortcutRecorder
        :model-value="binding.keys"
        :error="binding.error"
        @update:model-value="(v) => {
          binding.keys = v
          binding.error = undefined
        }"
      />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    :title="t('settings.group.appShortcuts')"
    :description="t('settings.description.appShortcuts')"
  >
    <SettingRow
      v-for="binding in appBindings"
      :key="binding.id"
      :title="bindingText(binding.id, 'label')"
      :description="bindingText(binding.id, 'description')"
    >
      <ShortcutRecorder
        :model-value="binding.keys"
        :error="binding.error"
        @update:model-value="(v) => {
          binding.keys = v
          binding.error = undefined
        }"
      />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    v-if="conflictedBindings.length"
    :title="t('settings.group.shortcutConflicts')"
    :description="t('settings.description.shortcutConflicts')"
  >
    <SettingRow
      v-for="binding in conflictedBindings"
      :key="binding.id"
      :title="bindingText(binding.id, 'label')"
      :description="binding.error ?? ''"
    >
      <span class="text-xs text-destructive">{{ t('settings.status.occupied') }}</span>
    </SettingRow>
  </SettingGroup>
</template>
