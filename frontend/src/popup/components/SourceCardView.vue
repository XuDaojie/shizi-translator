<script setup lang="ts">
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
    <div class="source-input" :title="text">{{ text || '输入要翻译的文本…' }}</div>
    <div class="source-meta">
      <button class="meta-btn" title="朗读原文" @click.stop="emit('speak')">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
      </button>
      <button class="meta-btn" title="复制原文" @click.stop="emit('copy')">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
      </button>
      <div class="meta-badges">
        <span class="lang-badge" :title="`检测到的源语言：${langLabel}`">{{ langLabel }}</span>
      </div>
    </div>
  </div>
</template>
