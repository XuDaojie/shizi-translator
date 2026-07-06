<script setup lang="ts">
import { Select } from '@/components/ui/select'
import type { SettingOption } from './types'

interface Props {
  modelValue: string
  options: SettingOption[]
  placeholder?: string
  disabled?: boolean
}

withDefaults(defineProps<Props>(), {
  placeholder: '请选择',
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()
</script>

<template>
  <Select
    :model-value="modelValue"
    :options="options"
    :placeholder="placeholder"
    :disabled="disabled"
    class="min-w-[168px]"
    @update:model-value="(v) => emit('update:modelValue', v)"
  />
</template>
