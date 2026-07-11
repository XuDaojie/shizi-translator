<script setup lang="ts">
import { computed } from 'vue'
import {
  SettingGroup,
  SettingRow,
  SettingSelect,
  SettingSwitch,
  SettingInput,
} from '../components'
import type { AppSettings } from '../types'
import { SOURCE_LANGUAGES, TARGET_LANGUAGES } from '@/shared/translation-languages'
const props = defineProps<{
  state: AppSettings
}>()

const sourceLanguageOptions = SOURCE_LANGUAGES.map((l) => ({ label: l.nativeName, value: l.code }))
const targetLanguageOptions = TARGET_LANGUAGES.map((l) => ({ label: l.nativeName, value: l.code }))
</script>

<template>
  <SettingGroup
    title="默认语言"
    description="划词 / 截图 / 剪贴板翻译时使用的源语言与目标语言。"
  >
    <SettingRow title="默认源语言" description="设置后,翻译时不再进行自动检测。">
      <SettingSelect
        v-model="state.translation.defaultSourceLang"
        :options="sourceLanguageOptions"
      />
    </SettingRow>
    <SettingRow title="默认目标语言" description="最常用的目标语种,可在翻译时临时切换。">
      <SettingSelect v-model="state.translation.defaultTargetLang" :options="targetLanguageOptions" />
    </SettingRow>
    <SettingRow
      title="自动检测语种"
      description="对源语言无法直接判断时启用。"
    >
      <SettingSwitch v-model="state.translation.autoDetect" aria-label="自动检测语种" />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    title="翻译行为"
    description="控制结果展示、剪贴板与自动粘贴等自动化行为。"
  >
    <SettingRow
      title="自动复制结果"
      description="翻译完成后将译文复制到剪贴板,便于粘贴到目标应用。"
    >
      <SettingSwitch v-model="state.translation.autoCopy" aria-label="自动复制结果" />
    </SettingRow>
    <SettingRow
      title="翻译后恢复原剪贴板"
      description="将原文放回剪贴板,避免覆盖正在编辑的内容。"
    >
      <SettingSwitch
        v-model="state.translation.restoreClipboard"
        aria-label="翻译后恢复原剪贴板"
      />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    title="取词与历史"
    description="调整取词响应速度与历史记录的存储上限。"
  >
    <SettingRow
      title="历史记录上限"
      description="超过上限的旧记录会被自动清理。"
    >
      <SettingInput
        v-model="state.translation.historyLimit"
        type="number"
        placeholder="500"
      />
    </SettingRow>
  </SettingGroup>
</template>
