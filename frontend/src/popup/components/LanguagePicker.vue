<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { SOURCE_LANGUAGES, TARGET_LANGUAGES, type TranslationLanguage } from '@/shared/translation-languages'
import { t, type MessageKey } from '@/i18n'

interface Props {
  modelValue: string
  type: 'source' | 'target'
  placeholder: string
}

const props = defineProps<Props>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'pick', value: string): void
}>()

const search = ref('')
const listRef = ref<HTMLUListElement | null>(null)
const inputRef = ref<HTMLInputElement | null>(null)

const filtered = computed<TranslationLanguage[]>(() => {
  const q = search.value.trim().toLowerCase()
  return (props.type === 'source' ? SOURCE_LANGUAGES : TARGET_LANGUAGES).filter((l) => {
    if (!q) return true
    return l.nativeName.toLowerCase().includes(q) || t(l.nameKey as MessageKey).toLowerCase().includes(q) || l.promptName.toLowerCase().includes(q)
  })
})

const select = (value: string): void => {
  emit('update:modelValue', value)
  emit('pick', value)
}

const moveActive = (delta: 1 | -1): void => {
  if (!listRef.value) return
  const items = Array.from(listRef.value.querySelectorAll<HTMLElement>('.lang-option'))
  if (items.length === 0) return
  const currentIdx = items.findIndex((el) => el.classList.contains('is-active'))
  const nextIdx = Math.max(0, Math.min(items.length - 1, currentIdx + delta))
  items.forEach((el) => el.classList.remove('is-active'))
  items[nextIdx]?.classList.add('is-active')
  items[nextIdx]?.scrollIntoView({ block: 'nearest' })
}

const onKeydown = (e: KeyboardEvent): void => {
  if (e.key === 'ArrowDown') { e.preventDefault(); moveActive(1) }
  else if (e.key === 'ArrowUp') { e.preventDefault(); moveActive(-1) }
  else if (e.key === 'Enter') {
    e.preventDefault()
    const active = listRef.value?.querySelector<HTMLElement>('.lang-option.is-active')
    if (active) {
      const value = active.dataset.value
      if (value) select(value)
    }
  }
}

const setInitialActive = async (): Promise<void> => {
  await nextTick()
  if (!listRef.value) return
  const selected = listRef.value.querySelector<HTMLElement>('.lang-option.is-selected')
  ;(selected || listRef.value.querySelector<HTMLElement>('.lang-option'))?.classList.add('is-active')
}

watch(() => props.modelValue, () => { void setInitialActive() })

defineExpose({ focus: () => inputRef.value?.focus() })
</script>

<template>
  <div class="lang-picker">
    <div class="lang-picker-search">
      <svg class="lang-picker-search-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7" /><line x1="20" y1="20" x2="16.65" y2="16.65" /></svg>
      <input
        ref="inputRef"
        v-model="search"
        type="text"
        class="lang-picker-input"
        :placeholder="placeholder"
        autocomplete="off"
        spellcheck="false"
        @keydown="onKeydown"
      />
    </div>
    <ul ref="listRef" class="lang-picker-list">
      <li
        v-for="lang in filtered"
        :key="lang.code"
        class="lang-option"
        :class="{ 'is-selected': lang.code === modelValue }"
        :data-value="lang.code"
        @click="select(lang.code)"
      >
        <span class="lang-option-native">{{ lang.nativeName }}</span>
        <span class="lang-option-english">{{ t(lang.nameKey as MessageKey) }}</span>
      </li>
    </ul>
  </div>
</template>
