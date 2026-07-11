<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ChevronDown } from '@lucide/vue'
import { cn } from '@/lib/utils'
import type { ServiceMeta } from '../types'
import { t } from '@/i18n'

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: ServiceMeta[]
    placeholder?: string
    disabled?: boolean
    className?: string
  }>(),
  {
    disabled: false,
    className: '',
  },
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  /** 输入了一个不在列表中的名字,父组件应调用 addCustomServiceType 并把新 id 写回 v-model */
  (e: 'create', name: string): void
}>()

const open = ref(false)
const inputValue = ref(displayName(props.options, props.modelValue))
const containerRef = ref<HTMLElement | null>(null)
const inputRef = ref<HTMLInputElement | null>(null)
const activeIndex = ref(-1)

function displayName(list: ServiceMeta[], id: string): string {
  if (!id) return ''
  return list.find((o) => o.id === id)?.name ?? id
}

watch(
  () => [props.modelValue, props.options] as const,
  ([id, list]) => {
    // 仅在 v-model 外部变化时同步输入框,避免覆盖用户正在编辑
    if (inputValue.value.trim() === '' || props.options.some((o) => o.name === inputValue.value && o.id !== props.modelValue)) {
      inputValue.value = displayName(list, id)
    } else if (!open.value) {
      inputValue.value = displayName(list, id)
    }
  },
  { deep: true },
)

const filteredOptions = computed(() => {
  const q = inputValue.value.trim().toLowerCase()
  if (!q) return props.options
  return props.options.filter((o) => o.name.toLowerCase().includes(q))
})

const canCreateCustom = computed(() => {
  const v = inputValue.value.trim()
  if (!v) return false
  return !props.options.some((o) => o.name.toLowerCase() === v.toLowerCase())
})

const showHint = computed(
  () => filteredOptions.value.length === 0 && !canCreateCustom.value,
)

function openDropdown(): void {
  if (props.disabled) return
  if (!open.value) {
    open.value = true
    activeIndex.value = -1
  }
}

function onFocus(): void {
  openDropdown()
}

function onClick(): void {
  if (!open.value) openDropdown()
}

function onInput(): void {
  if (!open.value) openDropdown()
  activeIndex.value = -1
}

function selectOption(opt: ServiceMeta): void {
  inputValue.value = opt.name
  emit('update:modelValue', opt.id)
  open.value = false
}

function commitCustom(): void {
  const v = inputValue.value.trim()
  if (!v) return
  // 通知父组件注册新渠道;父组件拿到新 id 后会通过 v-model 写回,触发 watch 同步输入框
  emit('create', v)
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    if (!open.value) openDropdown()
    if (filteredOptions.value.length > 0) {
      activeIndex.value = (activeIndex.value + 1) % filteredOptions.value.length
    }
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    if (!open.value) openDropdown()
    if (filteredOptions.value.length > 0) {
      activeIndex.value =
        activeIndex.value <= 0
          ? filteredOptions.value.length - 1
          : activeIndex.value - 1
    }
  } else if (e.key === 'Enter') {
    e.preventDefault()
    if (activeIndex.value >= 0 && filteredOptions.value[activeIndex.value]) {
      selectOption(filteredOptions.value[activeIndex.value])
    } else if (canCreateCustom.value) {
      commitCustom()
      open.value = false
    } else if (filteredOptions.value.length > 0) {
      selectOption(filteredOptions.value[0])
    }
  } else if (e.key === 'Escape') {
    if (open.value) {
      open.value = false
      inputValue.value = displayName(props.options, props.modelValue)
      e.preventDefault()
      e.stopPropagation()
    }
  } else if (e.key === 'Tab') {
    if (canCreateCustom.value) {
      commitCustom()
    }
  }
}

function onBlur(): void {
  window.setTimeout(() => {
    if (canCreateCustom.value) {
      commitCustom()
    } else {
      // 回滚到当前 modelValue 对应的 name
      const expected = displayName(props.options, props.modelValue)
      if (inputValue.value !== expected) inputValue.value = expected
    }
    open.value = false
  }, 120)
}

function onOptionMouseDown(e: MouseEvent, opt: ServiceMeta): void {
  e.preventDefault()
  selectOption(opt)
}

function onCustomMouseDown(e: MouseEvent): void {
  e.preventDefault()
  commitCustom()
  open.value = false
}

function onDocumentMouseDown(e: MouseEvent): void {
  if (!open.value) return
  if (containerRef.value && !containerRef.value.contains(e.target as Node)) {
    open.value = false
  }
}

onMounted(() => document.addEventListener('mousedown', onDocumentMouseDown))
onUnmounted(() => document.removeEventListener('mousedown', onDocumentMouseDown))
</script>

<template>
  <div
    :class="cn('relative w-full min-w-[240px]', className)"
    ref="containerRef"
  >
    <div class="relative">
      <input
        ref="inputRef"
        v-model="inputValue"
        type="text"
        :placeholder="placeholder ?? t('settings.placeholder.channel')"
        :disabled="disabled"
        :class="
          cn(
            'flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 pr-9 text-sm shadow-sm transition-colors duration-150',
            'placeholder:text-muted-foreground',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1',
            'disabled:cursor-not-allowed disabled:opacity-50',
          )
        "
        autocomplete="off"
        spellcheck="false"
        @focus="onFocus"
        @click="onClick"
        @input="onInput"
        @keydown="onKeydown"
        @blur="onBlur"
      />
      <div class="pointer-events-none absolute inset-y-0 right-2 flex items-center">
        <ChevronDown
          :class="
            cn(
              'h-4 w-4 text-muted-foreground transition-transform duration-150',
              open && 'rotate-180',
            )
          "
        />
      </div>
    </div>

    <div
      v-if="open"
      class="absolute left-0 right-0 top-full z-50 mt-1 overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-md"
    >
      <ul
        v-if="filteredOptions.length"
        class="max-h-60 overflow-y-auto py-1 scrollbar-thin"
      >
        <li
          v-for="(opt, idx) in filteredOptions"
          :key="opt.id"
          :class="
            cn(
              'flex cursor-pointer select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none transition-colors duration-100',
              'hover:bg-accent hover:text-accent-foreground',
              idx === activeIndex && 'bg-accent text-accent-foreground',
              opt.id === modelValue && idx !== activeIndex && 'bg-muted/60',
            )
          "
          @mousedown="(e) => onOptionMouseDown(e, opt)"
        >
          <span class="flex-1 truncate">{{ opt.name }}</span>
          <span
            v-if="!opt.builtin"
            class="rounded bg-muted px-1 py-0.5 text-[9px] text-muted-foreground"
          >
            {{ t('settings.status.custom') }}
          </span>
        </li>
      </ul>

      <button
        v-if="canCreateCustom"
        type="button"
        class="flex w-full items-center gap-2 border-t border-border px-2.5 py-1.5 text-left text-sm text-primary transition-colors hover:bg-accent"
        @mousedown="onCustomMouseDown"
      >
        <span class="truncate">
          {{ t('settings.combobox.createChannel', { name: inputValue.trim() }) }}
        </span>
      </button>

      <div
        v-else-if="showHint"
        class="px-2.5 py-3 text-center text-xs text-muted-foreground"
      >
        {{ inputValue.trim() ? t('settings.combobox.noMatchingChannel') : t('settings.combobox.chooseChannel') }}
      </div>
    </div>
  </div>
</template>
