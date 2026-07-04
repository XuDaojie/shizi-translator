<script setup lang="ts">
import { SettingGroup, SettingRow, ShortcutRecorder } from '../components'
import type { AppSettings } from '../types'

defineProps<{
  state: AppSettings
}>()
</script>

<template>
  <SettingGroup
    title="全局快捷键"
    description="点按输入框,按下想要设置的组合键,Esc 取消。"
  >
    <SettingRow
      v-for="binding in state.shortcut.bindings"
      :key="binding.id"
      :title="binding.label"
      :description="binding.description"
      :status="binding.id === 'word-lookup' ? 'planned' : undefined"
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
    title="冲突提示"
    description="若快捷键已被系统或其他应用占用,设置面板会给出提示。"
  >
    <SettingRow
      title="检测系统快捷键占用"
      description="注册前先扫描常见系统快捷键,避免与系统手势冲突。"
    >
      <span class="text-xs text-muted-foreground">始终启用</span>
    </SettingRow>
  </SettingGroup>
</template>
