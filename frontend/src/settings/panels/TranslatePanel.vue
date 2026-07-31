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
import { t, type MessageKey } from '@/i18n'
const props = defineProps<{
  state: AppSettings
}>()

const sourceLanguageOptions = computed(() => SOURCE_LANGUAGES.map((l) => ({ label: t(l.nameKey as MessageKey), value: l.code })))
const targetLanguageOptions = computed(() => TARGET_LANGUAGES.map((l) => ({ label: t(l.nameKey as MessageKey), value: l.code })))
</script>

<template>
  <SettingGroup
    :title="t('settings.group.defaultLanguages')"
    :description="t('settings.description.defaultLanguages')"
  >
    <SettingRow :title="t('settings.field.defaultSourceLanguage')" :description="t('settings.description.defaultSourceLanguage')">
      <SettingSelect
        v-model="state.translation.defaultSourceLang"
        :options="sourceLanguageOptions"
      />
    </SettingRow>
    <SettingRow :title="t('settings.field.defaultTargetLanguage')" :description="t('settings.description.defaultTargetLanguage')">
      <SettingSelect v-model="state.translation.defaultTargetLang" :options="targetLanguageOptions" />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.autoDetectLanguage')"
      :description="t('settings.description.autoDetectLanguage')"
    >
      <SettingSwitch v-model="state.translation.autoDetect" :aria-label="t('settings.field.autoDetectLanguage')" />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    :title="t('settings.group.translationBehavior')"
    :description="t('settings.description.translationBehavior')"
  >
    <SettingRow
      :title="t('settings.field.autoCopy')"
      :description="t('settings.description.autoCopy')"
    >
      <SettingSwitch v-model="state.translation.autoCopy" :aria-label="t('settings.field.autoCopy')" />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.restoreClipboard')"
      :description="t('settings.description.restoreClipboard')"
    >
      <SettingSwitch
        v-model="state.translation.restoreClipboard"
        :aria-label="t('settings.field.restoreClipboard')"
      />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.markdownRender')"
      :description="t('settings.description.markdownRender')"
    >
      <SettingSwitch
        v-model="state.translation.markdownRender"
        :aria-label="t('settings.field.markdownRender')"
      />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    :title="t('settings.group.lookupHistory')"
    :description="t('settings.description.lookupHistory')"
  >
    <SettingRow
      :title="t('settings.field.historyLimit')"
      :description="t('settings.description.historyLimit')"
    >
      <SettingInput
        v-model="state.translation.historyLimit"
        type="number"
        placeholder="500"
      />
    </SettingRow>
  </SettingGroup>

  <SettingGroup
    :title="t('settings.group.popupDisplay')"
    :description="t('settings.description.popupDisplay')"
  >
    <SettingRow
      :title="t('settings.field.showCloseButton')"
      :description="t('settings.description.showCloseButton')"
    >
      <SettingSwitch
        v-model="state.translation.showCloseButton"
        :aria-label="t('settings.field.showCloseButton')"
      />
    </SettingRow>
    <SettingRow
      :title="t('settings.field.showInTaskbar')"
      :description="t('settings.description.showInTaskbar')"
    >
      <SettingSwitch
        v-model="state.translation.showInTaskbar"
        :aria-label="t('settings.field.showInTaskbar')"
      />
    </SettingRow>
  </SettingGroup>
</template>
