<script setup lang="ts">
import { computed, ref } from 'vue'
import { Eye, EyeOff, Loader2, ShieldCheck, ShieldAlert } from '@lucide/vue'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { t } from '@/i18n'

type Status = 'idle' | 'validating' | 'valid' | 'invalid'

interface Props {
  modelValue: string
  placeholder?: string
  status?: Status
  disabled?: boolean
  allowReveal?: boolean
  allowValidate?: boolean
  className?: string
}

const props = withDefaults(defineProps<Props>(), {
  status: 'idle',
  disabled: false,
  allowReveal: true,
  allowValidate: true,
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'validate', value: string): void
}>()

const revealed = ref(false)

const onInput = (value: string): void => emit('update:modelValue', value)
const onValidate = (): void => emit('validate', props.modelValue)

const isValidating = computed(() => props.status === 'validating')

const buttonTitle = computed(() => {
  switch (props.status) {
    case 'validating': return t('settings.status.validating')
    case 'valid':      return t('settings.status.valid')
    case 'invalid':    return t('settings.tooltip.retryValidation')
    default:           return t('settings.tooltip.validateApiKey')
  }
})

const buttonAria = computed(() => {
  switch (props.status) {
    case 'validating': return t('settings.status.validating')
    case 'valid':      return t('settings.status.valid')
    case 'invalid':    return t('common.retry')
    default:           return t('settings.button.validate')
  }
})
</script>

<template>
  <div :class="cn('flex w-full items-stretch', props.className)">
    <div class="relative flex-1">
      <Input
        :model-value="modelValue"
        :type="revealed ? 'text' : 'password'"
        :placeholder="placeholder ?? t('settings.placeholder.apiKey')"
        :disabled="disabled"
        :class="cn(
          'rounded-r-none border-r-0 font-mono',
          'focus-visible:ring-0 focus-visible:ring-offset-0',
          allowReveal ? 'pr-10' : 'pr-3',
        )"
        @update:model-value="onInput"
      />
      <button
        v-if="allowReveal"
        type="button"
        :disabled="disabled"
        :title="t('settings.tooltip.revealApiKey')"
        :aria-label="t('settings.tooltip.revealApiKey')"
        class="pointer-events-auto absolute inset-y-0 right-2 my-auto inline-flex h-6 w-6 items-center justify-center rounded text-muted-foreground/70 transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
        @click="revealed = !revealed"
        @mousedown.stop
      >
        <EyeOff v-if="revealed" class="h-3.5 w-3.5" />
        <Eye v-else class="h-3.5 w-3.5" />
      </button>
    </div>

    <button
      v-if="allowValidate"
      type="button"
      :disabled="disabled || !modelValue || isValidating"
      :title="buttonTitle"
      :aria-label="buttonAria"
      :class="cn(
        'relative inline-flex h-9 min-w-[3rem] shrink-0 items-center justify-center gap-1.5 overflow-hidden rounded-r-md border border-l-0 border-input bg-background px-3 text-sm text-muted-foreground transition-colors',
        'hover:bg-accent/50 hover:text-foreground',
        'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-offset-1',
        'disabled:pointer-events-none disabled:opacity-60',
        status === 'invalid' && 'border-destructive/60 text-destructive hover:bg-destructive/5 hover:text-destructive',
      )"
      @click="onValidate"
    >
      <template v-if="isValidating">
        <Loader2 class="h-3.5 w-3.5 animate-spin" />
      </template>
      <ShieldCheck v-else-if="status === 'valid'" class="h-4 w-4 text-emerald-500" />
      <ShieldAlert v-else-if="status === 'invalid'" class="h-4 w-4" />
      <span v-else>{{ t('settings.button.validate') }}</span>
    </button>
  </div>
</template>
