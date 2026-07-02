<script setup lang="ts">
import { computed } from 'vue'
import { RotateCcw } from '@lucide/vue'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'

interface Props {
  modelValue: string
  placeholder?: string
  defaultValue?: string
  /** 最小行数,默认 3。 */
  minRows?: number
  /** 最大行数,超过后内容可滚动。默认 8。 */
  maxRows?: number
  disabled?: boolean
  /** 是否显示右上角「重置」按钮;在 defaultValue 不为空且与当前值不同时显示。 */
  showReset?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  minRows: 3,
  maxRows: 8,
  disabled: false,
  showReset: true,
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'reset'): void
}>()

const charCount = computed(() => props.modelValue.length)

const isDirty = computed(
  () =>
    props.showReset &&
    props.defaultValue !== undefined &&
    props.modelValue !== props.defaultValue,
)

const lineHeightRem = 1.5
const minHeight = computed(() => `${props.minRows * lineHeightRem + 1.25}rem`)
const maxHeight = computed(() => `${props.maxRows * lineHeightRem + 1.25}rem`)

const onInput = (e: Event): void => {
  const target = e.target as HTMLTextAreaElement
  emit('update:modelValue', target.value)
}

const onReset = (): void => {
  if (props.defaultValue === undefined) return
  emit('update:modelValue', props.defaultValue)
  emit('reset')
}
</script>

<template>
  <div class="w-full">
    <div class="flex items-start justify-between gap-2">
      <textarea
        :value="modelValue"
        :placeholder="placeholder"
        :disabled="disabled"
        :rows="minRows"
        :class="
          cn(
            'w-full min-w-0 resize-y rounded-md border border-input bg-background px-3 py-2',
            'text-sm leading-relaxed text-foreground placeholder:text-muted-foreground/70',
            'transition-colors duration-150',
            'hover:border-muted-foreground/40',
            'focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary',
            'disabled:cursor-not-allowed disabled:opacity-60',
            'font-mono',
          )
        "
        :style="{ minHeight, maxHeight }"
        @input="onInput"
      />
      <Button
        v-if="isDirty"
        type="button"
        variant="ghost"
        size="icon"
        class="h-7 w-7 shrink-0 text-muted-foreground hover:text-foreground"
        title="重置为默认"
        aria-label="重置为默认"
        @click="onReset"
      >
        <RotateCcw class="h-3.5 w-3.5" />
      </Button>
    </div>
    <div class="mt-1 flex justify-end text-[10px] tabular-nums text-muted-foreground/70">
      {{ charCount }} 字符
    </div>
  </div>
</template>
