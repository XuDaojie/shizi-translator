<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { speakText, copyText } from '../composables/utils'
import { t } from '@/i18n'

interface Props {
  modelValue: string
  langLabel: string
  sourceBadge?: 'selectedText' | 'ocrText' | null
  detectedLang?: string
}

const props = withDefaults(defineProps<Props>(), { sourceBadge: null, detectedLang: '' })
const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'submit'): void
  (e: 'input'): void
}>()

const textareaRef = ref<HTMLTextAreaElement | null>(null)
const copied = ref(false)

const sourceBadgeText = computed(() => {
  switch (props.sourceBadge) {
    case 'selectedText': return t('popup.source.selection')
    case 'ocrText': return t('popup.source.ocr')
    default: return ''
  }
})

const autoResize = (): void => {
  const el = textareaRef.value
  if (!el) return
  el.style.height = 'auto'
  const maxHeight = parseFloat(getComputedStyle(el).maxHeight)
  const nextHeight = Math.min(el.scrollHeight, maxHeight || el.scrollHeight)
  el.style.height = nextHeight + 'px'
  el.style.overflowY = el.scrollHeight > nextHeight ? 'auto' : 'hidden'
}

const onInput = (e: Event): void => {
  const value = (e.target as HTMLTextAreaElement).value
  emit('update:modelValue', value)
  emit('input')
  autoResize()
}

const onKeydown = (e: KeyboardEvent): void => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    emit('submit')
  }
}

const onSpeak = (): void => {
  speakText(props.modelValue, 'en-US')
}

const onCopy = async (): Promise<void> => {
  const ok = await copyText(props.modelValue)
  if (ok) {
    copied.value = true
    setTimeout(() => { copied.value = false }, 1500)
  }
}

onMounted(() => {
  autoResize()
  if (typeof document !== 'undefined' && document.fonts) {
    document.fonts.ready.then(autoResize).catch(() => {})
  }
})

watch(() => props.modelValue, () => { nextTick(autoResize) })

defineExpose({ focus: () => textareaRef.value?.focus(), autoResize })
</script>

<template>
  <div class="source-card">
    <textarea
      ref="textareaRef"
      class="source-input"
      :value="modelValue"
      dir="auto"
      :placeholder="t('popup.placeholder.source')"
      rows="3"
      @input="onInput"
      @keydown="onKeydown"
    />
    <div class="source-meta">
      <button class="meta-btn" :title="t('popup.tooltip.speakSource')" :aria-label="t('popup.tooltip.speakSource')" @click="onSpeak">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
      </button>
      <button class="meta-btn" :class="{ copied }" :title="t('popup.tooltip.copySource')" :aria-label="t('popup.tooltip.copySource')" @click="onCopy">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
      </button>
      <div class="meta-badges">
        <span v-if="sourceBadgeText" class="source-badge">{{ sourceBadgeText }}</span>
        <span v-if="detectedLang" class="lang-badge">{{ detectedLang }}</span>
        <span v-else-if="langLabel" class="lang-badge">{{ langLabel }}</span>
      </div>
    </div>
  </div>
</template>
