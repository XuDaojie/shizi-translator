<script setup lang="ts">
import { computed } from 'vue'
import { SettingGroup, SettingRow, ShortcutRecorder } from '../components'
import type { AppSettings } from '../types'

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
</script>

<template>
  <SettingGroup
    title="全局快捷键"
    description="点按输入框，按下想要设置的组合键，Esc 取消。全局快捷键在应用未聚焦时也可用。"
  >
    <SettingRow
      v-for="binding in globalBindings"
      :key="binding.id"
      :title="binding.label"
      :description="binding.description"
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
    title="程序快捷键"
    description="仅在本应用窗口聚焦时生效，不会占用系统全局快捷键。"
  >
    <SettingRow
      v-for="binding in appBindings"
      :key="binding.id"
      :title="binding.label"
      :description="binding.description"
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
    title="冲突提示"
    description="以下快捷键已被系统或其他应用占用，请在对应行修改或清空。"
  >
    <SettingRow
      v-for="binding in conflictedBindings"
      :key="binding.id"
      :title="binding.label"
      :description="binding.error ?? ''"
    >
      <span class="text-xs text-destructive">占用</span>
    </SettingRow>
  </SettingGroup>
</template>
