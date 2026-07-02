<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ChevronDown, Loader2 } from '@lucide/vue'
import { cn } from '@/lib/utils'

const props = withDefaults(
  defineProps<{
    modelValue: string
    models: string[]
    loading?: boolean
    placeholder?: string
    disabled?: boolean
    className?: string
  }>(),
  {
    loading: false,
    placeholder: '请选择或输入模型名',
    disabled: false,
    className: '',
  },
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  /** 下拉打开时触发（首次打开会触发拉取） */
  (e: 'open'): void
}>()

const open = ref(false)
const inputValue = ref(props.modelValue)
const containerRef = ref<HTMLElement | null>(null)
const inputRef = ref<HTMLInputElement | null>(null)
const activeIndex = ref(-1)

watch(
  () => props.modelValue,
  (val) => {
    if (val !== inputValue.value) inputValue.value = val
  },
)

const filteredModels = computed(() => {
  const q = inputValue.value.trim().toLowerCase()
  if (!q) return props.models
  return props.models.filter((m) => m.toLowerCase().includes(q))
})

const canCommitCustom = computed(() => {
  const v = inputValue.value.trim()
  return v.length > 0 && !props.models.includes(v)
})

const showHint = computed(
  () => !props.loading && filteredModels.value.length === 0 && !canCommitCustom.value,
)

const showLoadingHint = computed(() => props.loading && filteredModels.value.length === 0)

function openDropdown(): void {
  if (props.disabled) return
  if (!open.value) {
    open.value = true
    activeIndex.value = -1
    emit('open')
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

function selectModel(model: string): void {
  inputValue.value = model
  emit('update:modelValue', model)
  activeIndex.value = -1
}

function commitCustom(): void {
  const v = inputValue.value.trim()
  if (!v) return
  emit('update:modelValue', v)
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    if (!open.value) openDropdown()
    if (filteredModels.value.length > 0) {
      activeIndex.value = (activeIndex.value + 1) % filteredModels.value.length
    }
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    if (!open.value) openDropdown()
    if (filteredModels.value.length > 0) {
      activeIndex.value =
        activeIndex.value <= 0
          ? filteredModels.value.length - 1
          : activeIndex.value - 1
    }
  } else if (e.key === 'Enter') {
    e.preventDefault()
    if (activeIndex.value >= 0 && filteredModels.value[activeIndex.value]) {
      selectModel(filteredModels.value[activeIndex.value])
    } else if (canCommitCustom.value) {
      commitCustom()
      open.value = false
    } else if (filteredModels.value.length > 0) {
      selectModel(filteredModels.value[0])
    }
  } else if (e.key === 'Escape') {
    if (open.value) {
      open.value = false
      inputValue.value = props.modelValue
      e.preventDefault()
      e.stopPropagation()
    }
  } else if (e.key === 'Tab') {
    if (canCommitCustom.value) commitCustom()
  }
}

function onBlur(): void {
  // 延时关闭，以便 option 的 mousedown 先触发
  window.setTimeout(() => {
    if (canCommitCustom.value) {
      commitCustom()
    } else if (inputValue.value !== props.modelValue && filteredModels.value.some((m) => m === inputValue.value)) {
      emit('update:modelValue', inputValue.value)
    } else if (inputValue.value !== props.modelValue) {
      // 用户键入了不匹配的内容，回滚
      inputValue.value = props.modelValue
    }
    open.value = false
  }, 120)
}

function onOptionMouseDown(e: MouseEvent, model: string): void {
  e.preventDefault()
  selectModel(model)
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
        :placeholder="placeholder"
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
      <div class="absolute inset-y-0 right-1.5 flex items-center">
        <div class="pointer-events-none flex items-center pr-1">
          <Loader2
            v-if="loading"
            class="h-3.5 w-3.5 animate-spin text-muted-foreground"
          />
          <ChevronDown
            v-else
            :class="
              cn(
                'h-4 w-4 text-muted-foreground transition-transform duration-150',
                open && 'rotate-180',
              )
            "
          />
        </div>
      </div>
    </div>

    <div
      v-if="open"
      class="absolute left-0 right-0 top-full z-50 mt-1 overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-md"
    >
      <ul
        v-if="filteredModels.length"
        class="max-h-60 overflow-y-auto py-1 scrollbar-thin"
      >
        <li
          v-for="(m, idx) in filteredModels"
          :key="m"
          :class="
            cn(
              'flex cursor-pointer select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none transition-colors duration-100',
              'hover:bg-accent hover:text-accent-foreground',
              idx === activeIndex && 'bg-accent text-accent-foreground',
              m === modelValue && idx !== activeIndex && 'bg-muted/60',
            )
          "
          @mousedown="(e) => onOptionMouseDown(e, m)"
        >
          <span class="flex-1 truncate">{{ m }}</span>
        </li>
      </ul>

      <button
        v-if="canCommitCustom"
        type="button"
        class="flex w-full items-center gap-2 border-t border-border px-2.5 py-1.5 text-left text-sm text-primary transition-colors hover:bg-accent"
        @mousedown="onCustomMouseDown"
      >
        <span class="truncate">
          使用自定义模型 "<span class="font-medium">{{ inputValue.trim() }}</span>"
        </span>
      </button>

      <div
        v-if="showLoadingHint"
        class="px-2.5 py-3 text-center text-xs text-muted-foreground"
      >
        正在拉取模型列表…
      </div>

      <div
        v-else-if="showHint"
        class="px-2.5 py-3 text-center text-xs text-muted-foreground"
      >
        {{ inputValue.trim() ? '没有匹配的模型，可直接使用上方输入' : '请输入或选择模型' }}
      </div>

      <div v-if="loading" class="loading-bar" />
    </div>
  </div>
</template>

<style scoped>
.loading-bar {
  position: relative;
  height: 2px;
  background: hsl(var(--muted));
  overflow: hidden;
}
.loading-bar::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  width: 30%;
  background: hsl(var(--primary));
  box-shadow: 0 0 6px 0 hsl(var(--primary) / 0.4);
  animation: combobox-indeterminate 1.1s cubic-bezier(0.65, 0.815, 0.735, 0.395) infinite;
}
@keyframes combobox-indeterminate {
  0% {
    left: -30%;
  }
  100% {
    left: 100%;
  }
}
</style>
