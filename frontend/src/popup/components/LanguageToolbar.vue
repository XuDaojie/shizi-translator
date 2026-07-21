<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { translationLanguage } from '@/shared/translation-languages'
import LanguagePicker from './LanguagePicker.vue'
import { t } from '@/i18n'

interface Props {
  source: string
  target: string
  readonly?: boolean
}

const props = withDefaults(defineProps<Props>(), { readonly: false })

const emit = defineEmits<{
  (e: 'update:source', value: string): void
  (e: 'update:target', value: string): void
  (e: 'swap'): void
}>()

const sourceLabel = computed(() => translationLanguage(props.source)?.nativeName ?? props.source)
const targetLabel = computed(() => translationLanguage(props.target)?.nativeName ?? props.target)

const openType = ref<'source' | 'target' | null>(null)
const toolbarRef = ref<HTMLElement | null>(null)
const sourcePickerRef = ref<InstanceType<typeof LanguagePicker> | null>(null)
const targetPickerRef = ref<InstanceType<typeof LanguagePicker> | null>(null)

const toggle = (type: 'source' | 'target'): void => {
  if (props.readonly) return
  if (openType.value === type) { openType.value = null; return }
  openType.value = type
  requestAnimationFrame(() => {
    if (type === 'source') sourcePickerRef.value?.focus()
    else targetPickerRef.value?.focus()
  })
}

const onPick = (type: 'source' | 'target', value: string): void => {
  openType.value = null
  if (type === 'source') emit('update:source', value)
  else emit('update:target', value)
}

const swap = (): void => {
  if (props.readonly) return
  openType.value = null
  emit('swap')
}

const onDocClick = (e: MouseEvent): void => {
  if (!openType.value) return
  const target = e.target as HTMLElement
  if (target.closest('.lang-toolbar') || target.closest('.lang-picker')) return
  openType.value = null
}

watch(openType, (val) => {
  if (val) {
    setTimeout(() => document.addEventListener('click', onDocClick), 0)
  } else {
    document.removeEventListener('click', onDocClick)
  }
})

onBeforeUnmount(() => {
  document.removeEventListener('click', onDocClick)
})
</script>

<template>
  <div ref="toolbarRef" class="lang-toolbar">
    <button class="lang-side" type="button" :disabled="readonly" :aria-label="t('popup.aria.sourceLanguage')" @click="toggle('source')">
      <span class="lang-label">{{ sourceLabel }}</span>
      <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
    </button>
    <button class="lang-swap" type="button" :disabled="readonly" :title="t('popup.tooltip.swapLanguages')" :aria-label="t('popup.tooltip.swapLanguages')" @click="swap">
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M7 16l-4-4 4-4" /><path d="M17 8l4 4-4 4" /><line x1="3" y1="12" x2="21" y2="12" /></svg>
    </button>
    <button class="lang-side" type="button" :disabled="readonly" :aria-label="t('popup.aria.targetLanguage')" @click="toggle('target')">
      <span class="lang-label">{{ targetLabel }}</span>
      <svg class="lang-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
    </button>
  </div>
  <LanguagePicker
    v-if="openType === 'source'"
    ref="sourcePickerRef"
    :model-value="source"
    type="source"
    :placeholder="t('popup.placeholder.searchLanguage')"
    :anchor-el="toolbarRef"
    @pick="(v) => onPick('source', v)"
    @close="openType = null"
  />
  <LanguagePicker
    v-if="openType === 'target'"
    ref="targetPickerRef"
    :model-value="target"
    type="target"
    :placeholder="t('popup.placeholder.searchLanguage')"
    :anchor-el="toolbarRef"
    @pick="(v) => onPick('target', v)"
    @close="openType = null"
  />
</template>
