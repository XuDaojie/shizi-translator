<script setup lang="ts">
import { SettingGroup, SettingRow, ShortcutRecorder } from '../components'
import type { AppSettings } from '../types'

defineProps<{
  state: AppSettings
}>()

/** 后端硬编码、不可配置的快捷键 id（只读展示真实绑定）。 */
const READONLY_IDS = new Set(['translate-selection', 'translate-screenshot'])
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
      status="wip"
    >
      <ShortcutRecorder
        :model-value="binding.keys"
        :error="binding.error"
        :disabled="READONLY_IDS.has(binding.id)"
        @update:model-value="(v) => {
          binding.keys = v
          if (v) binding.error = undefined
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
