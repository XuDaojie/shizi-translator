<script setup lang="ts">
import { computed } from 'vue'
import { SettingGroup, SettingRow, ShortcutRecorder } from '../components'
import type { AppSettings } from '../types'

const props = defineProps<{
  state: AppSettings
}>()

const conflictedBindings = computed(() =>
  props.state.shortcut.bindings.filter((b) => b.error),
)
</script>

<template>
  <SettingGroup
    title="全局快捷键"
    description="点按输入框,按下想要设置的组合键,Esc 取消。"
  >
    <SettingRow
      v-for="binding in state.shortcut.bindings.filter((b) => b.id !== 'word-lookup')"
      :key="binding.id"
      :title="binding.label"
      :description="binding.description">
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



