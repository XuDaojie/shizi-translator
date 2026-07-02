<script setup lang="ts">
import { computed, useAttrs } from 'vue'
import { cn } from '@/lib/utils'
import { inputVariants, type InputVariants } from './index'

interface Props {
  modelValue?: string | number
  size?: InputVariants['size']
  invalid?: boolean
  class?: string
  type?: string
}

const props = withDefaults(defineProps<Props>(), {
  size: 'default',
  invalid: false,
  type: 'text',
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const attrs = useAttrs()
const classes = computed(() =>
  cn(inputVariants({ size: props.size, invalid: props.invalid }), props.class),
)
</script>

<template>
  <input
    :type="type"
    :value="modelValue"
    :class="classes"
    v-bind="attrs"
    @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
  />
</template>
