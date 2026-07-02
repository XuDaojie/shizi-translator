<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { AlertCircle, Keyboard, X } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

interface Props {
  modelValue: string
  placeholder?: string
  disabled?: boolean
  className?: string
  /** 绑定失败时的原因;非空时按钮变红、下方显示错误说明。 */
  error?: string
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: '点按后按下快捷键',
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
    <div class="flex items-center gap-2">
      <button
        type="button"
        :disabled="disabled"
        :class="
          cn(
            'inline-flex h-9 min-w-[120px] items-center justify-between gap-2 rounded-md border border-input bg-background px-3 text-sm shadow-sm',
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
          <span class="text-xs">按下快捷键 (Esc 取消)</span>
        </span>
        <span v-else-if="modelValue" class="font-mono text-foreground">
          {{ modelValue }}
        </span>
        <span v-else class="text-muted-foreground text-xs">
          {{ placeholder }}
        </span>
        <span v-if="recording" class="sr-only">正在录入</span>
      </button>

      <Button
        v-if="modelValue"
        variant="ghost"
        size="icon"
        :disabled="disabled"
        :title="disabled ? undefined : '清除快捷键'"
        aria-label="清除快捷键"
        @click="clear"
      >
        <X class="h-4 w-4" />
      </Button>
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
