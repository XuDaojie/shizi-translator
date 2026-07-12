<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { AlertCircle, Keyboard, X } from '@lucide/vue'
import { cn } from '@/lib/utils'
import { t } from '@/i18n'

interface Props {
  modelValue: string
  placeholder?: string
  disabled?: boolean
  className?: string
  /** 绑定失败时的原因;非空时按钮变红、下方显示错误说明。 */
  error?: string
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const recording = ref(false)

const isMac = computed(() =>
  typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform),
)

const formatKeys = (e: KeyboardEvent): string => {
  const parts: string[] = []
  const mod = isMac.value
  if (e.metaKey) parts.push('⌘')
  if (e.ctrlKey) parts.push(isMac.value ? '⌃' : 'Ctrl')
  if (e.altKey) parts.push(isMac.value ? '⌥' : 'Alt')
  if (e.shiftKey) parts.push(isMac.value ? '⇧' : 'Shift')
  const key = e.key
  if (!['Control', 'Shift', 'Alt', 'Meta'].includes(key)) {
    parts.push(key.length === 1 ? key.toUpperCase() : key)
  }
  return parts.join('+')
}

const onKeydown = (e: KeyboardEvent): void => {
  if (!recording.value) return
  e.preventDefault()
  e.stopPropagation()
  if (e.key === 'Escape') {
    recording.value = false
    return
  }
  // ponytail: 跳过纯修饰键 keydown，等实际按键到达再捕获完整组合键
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) {
    return
  }
  const combo = formatKeys(e)
  if (combo) {
    emit('update:modelValue', combo)
    recording.value = false
  }
}

const start = (): void => {
  if (props.disabled) return
  recording.value = true
  window.addEventListener('keydown', onKeydown, true)
}

const clear = (): void => {
  if (props.disabled) return
  emit('update:modelValue', '')
}

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown, true)
})

const onClickOutside = (): void => {
  if (recording.value) {
    recording.value = false
    window.removeEventListener('keydown', onKeydown, true)
  }
}
</script>

<template>
  <div :class="cn('flex flex-col items-end gap-1', props.className)" @click.stop>
    <div class="flex items-center">
      <button
        type="button"
        :disabled="disabled"
        :class="
          cn(
            'inline-flex h-9 min-w-[160px] items-center justify-between gap-1 rounded-md border border-input bg-background px-3 text-sm shadow-sm',
            'transition-colors duration-150 ease-smooth',
            'hover:bg-accent/40',
            'focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-1',
            'disabled:cursor-not-allowed disabled:opacity-50',
            recording && 'border-primary ring-2 ring-primary/40',
            error && 'border-destructive/70 bg-destructive/5 text-destructive',
          )
        "
        :aria-invalid="error ? 'true' : undefined"
        :title="error ?? undefined"
        @click="start"
        @blur="onClickOutside"
      >
        <span v-if="recording" class="flex items-center gap-2 text-primary">
          <Keyboard class="h-3.5 w-3.5 animate-pulse" />
          <span class="text-xs">{{ t('settings.shortcut.recording') }}</span>
        </span>
        <template v-else-if="modelValue">
          <span class="font-mono text-foreground">{{ modelValue }}</span>
          <span
            class="flex h-5 w-5 items-center justify-center rounded text-muted-foreground/40 transition-colors hover:bg-destructive/10 hover:text-destructive"
            :title="disabled ? undefined : t('settings.shortcut.clear')"
            :aria-label="t('settings.shortcut.clear')"
            @click.stop="clear"
          >
            <X class="h-3.5 w-3.5" />
          </span>
        </template>
        <span v-else class="text-muted-foreground text-xs">
          {{ placeholder ?? t('settings.placeholder.shortcut') }}
        </span>
        <span v-if="recording" class="sr-only">{{ t('settings.shortcut.recordingAria') }}</span>
      </button>
    </div>

    <p
      v-if="error"
      class="flex items-center gap-1.5 text-xs text-destructive"
      role="alert"
    >
      <AlertCircle class="h-3 w-3 shrink-0" />
      <span>{{ error }}</span>
    </p>
  </div>
</template>
