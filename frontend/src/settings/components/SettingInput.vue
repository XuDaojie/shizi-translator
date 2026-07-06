<script setup lang="ts">
import { Input } from '@/components/ui/input'

interface Props {
  modelValue: string | number
  placeholder?: string
  type?: string
  disabled?: boolean
  invalid?: boolean
}

withDefaults(defineProps<Props>(), {
  type: 'text',
  invalid: false,
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string | number): void
}>()

const onUpdate = (props: { type?: string }, raw: string): void => {
  // number 类型 input:parseFloat 后 emit,保持父组件状态为 number
  if (props.type === 'number') {
    const parsed = raw === '' ? 0 : Number(raw)
    emit('update:modelValue', Number.isFinite(parsed) ? parsed : 0)
    return
  }
  emit('update:modelValue', raw)
}
</script>

<template>
  <Input
    :model-value="modelValue"
    :type="type"
    :placeholder="placeholder"
    :disabled="disabled"
    :invalid="invalid"
    class="min-w-[190px]"
    @update:model-value="(v) => onUpdate({ type }, v)"
  />
</template>
