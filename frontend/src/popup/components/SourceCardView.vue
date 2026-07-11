<script setup lang="ts">
import { t } from '@/i18n'
interface Props {
  text: string
  langLabel: string
}
withDefaults(defineProps<Props>(), { text: '', langLabel: '' })
const emit = defineEmits<{
  (e: 'speak'): void
  (e: 'copy'): void
  (e: 'focus'): void
}>()
</script>

<template>
  <div class="source-card" @click="emit('focus')">
    <div class="source-input" dir="auto" :title="text">{{ text || t('popup.placeholder.source') }}</div>
    <div class="source-meta">
      <button class="meta-btn" :title="t('popup.tooltip.speakSource')" :aria-label="t('popup.tooltip.speakSource')" @click.stop="emit('speak')">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
      </button>
      <button class="meta-btn" :title="t('popup.tooltip.copySource')" :aria-label="t('popup.tooltip.copySource')" @click.stop="emit('copy')">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
      </button>
      <div class="meta-badges">
        <span class="lang-badge" :title="t('popup.aria.detectedLanguage', { language: langLabel })">{{ langLabel }}</span>
      </div>
    </div>
  </div>
</template>
