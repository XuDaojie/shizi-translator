<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { speakText, copyText, applyRemoveBlankIfActive } from '../composables/utils'
import { t } from '@/i18n'

interface Props {
  modelValue: string
  langLabel: string
  sourceBadge?: 'selectedText' | 'ocrText' | null
  detectedLang?: string
  /** 去除空行开关（由父组件持有，便于外部回填后重译） */
  removeBlankActive?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  sourceBadge: null,
  detectedLang: '',
  removeBlankActive: false,
})
const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'update:removeBlankActive', value: boolean): void
  (e: 'submit'): void
  /** 开启去除空行并改动了原文：父组件应强制用清洗后文本重译 */
  (e: 'retranslate'): void
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

const removeBlankTooltip = computed(() =>
  props.removeBlankActive
    ? t('popup.tooltip.removeBlankLinesOn')
    : t('popup.tooltip.removeBlankLines'),
)

const autoResize = (): void => {
  const el = textareaRef.value
  if (!el) return
  el.style.height = 'auto'
  const maxHeight = parseFloat(getComputedStyle(el).maxHeight)
  const nextHeight = Math.min(el.scrollHeight, maxHeight || el.scrollHeight)
  el.style.height = nextHeight + 'px'
  el.style.overflowY = el.scrollHeight > nextHeight ? 'auto' : 'hidden'
}

/** 若开启且仍有空行，写回父级；返回是否改动了文本。 */
const syncStrippedSource = (text: string): boolean => {
  const next = applyRemoveBlankIfActive(props.removeBlankActive, text)
  if (next === text) return false
  emit('update:modelValue', next)
  emit('input')
  return true
}

const onInput = (e: Event): void => {
  const raw = (e.target as HTMLTextAreaElement).value
  const value = applyRemoveBlankIfActive(props.removeBlankActive, raw)
  if (value !== raw && textareaRef.value) {
    textareaRef.value.value = value
  }
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

const onRemoveBlankLines = (): void => {
  if (props.removeBlankActive) {
    emit('update:removeBlankActive', false)
    return
  }
  emit('update:removeBlankActive', true)
  if (!props.modelValue) return
  const next = applyRemoveBlankIfActive(true, props.modelValue)
  if (next !== props.modelValue) {
    emit('update:modelValue', next)
    emit('input')
    // 父组件强制重译（含翻译中），结果卡才会跟清洗后的原文一致
    emit('retranslate')
  }
}

onMounted(() => {
  autoResize()
  if (typeof document !== 'undefined' && document.fonts) {
    document.fonts.ready.then(autoResize).catch(() => {})
  }
})

watch(
  () => props.modelValue,
  (text) => {
    // 外部回填时父组件也会清洗；此处兜底，避免展示短暂残留空行
    syncStrippedSource(text)
    nextTick(autoResize)
  },
)

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
      <button class="meta-btn" type="button" :title="t('popup.tooltip.speakSource')" :aria-label="t('popup.tooltip.speakSource')" @click="onSpeak">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
      </button>
      <button class="meta-btn" type="button" :class="{ copied }" :title="t('popup.tooltip.copySource')" :aria-label="t('popup.tooltip.copySource')" @click="onCopy">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
      </button>
      <button
        class="meta-btn"
        type="button"
        :class="{ active: removeBlankActive }"
        :title="removeBlankTooltip"
        :aria-label="removeBlankTooltip"
        :aria-pressed="removeBlankActive"
        @click="onRemoveBlankLines"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="m7 21-4.3-4.3c-1-1-1-2.5 0-3.4l9.6-9.6c1-1 2.5-1 3.4 0l5.6 5.6c1 1 1 2.5 0 3.4L13 21" /><path d="M22 21H7" /><path d="m5 11 9 9" /></svg>
      </button>
      <div class="meta-badges">
        <span v-if="sourceBadgeText" class="source-badge">{{ sourceBadgeText }}</span>
        <span v-if="detectedLang" class="lang-badge">{{ detectedLang }}</span>
        <span v-else-if="langLabel" class="lang-badge">{{ langLabel }}</span>
      </div>
    </div>
  </div>
</template>
