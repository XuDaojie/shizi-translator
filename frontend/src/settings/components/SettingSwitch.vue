<script setup lang="ts">
import { Switch } from '@/components/ui/switch'

interface Props {
  modelValue: boolean
  disabled?: boolean
  ariaLabel?: string
}

withDefaults(defineProps<Props>(), {
  disabled: false,
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()
</script>

<template>
  <Switch
    :model-value="modelValue"
    :disabled="disabled"
    :aria-label="ariaLabel"
    @update:model-value="(v) => emit('update:modelValue', v)"
  />
</template>
