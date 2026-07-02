<script setup lang="ts">
import { computed } from 'vue'
import { SwitchRoot, SwitchThumb, type SwitchRootProps } from 'reka-ui'
import { cn } from '@/lib/utils'

interface Props extends Omit<SwitchRootProps, 'class' | 'modelValue'> {
  modelValue?: boolean
  class?: string
  ariaLabel?: string
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: false,
  disabled: false,
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const onCheckedChange = (value: boolean): void => {
  emit('update:modelValue', value)
}

const rootClasses = computed(() =>
  cn(
    'peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent shadow-sm transition-colors duration-200 ease-smooth',
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2',
    'disabled:cursor-not-allowed disabled:opacity-50',
    'data-[state=checked]:bg-primary data-[state=unchecked]:bg-input',
    props.class,
  ),
)

const thumbClasses = computed(() =>
  cn(
    'pointer-events-none block h-4 w-4 rounded-full bg-background shadow-lg ring-0 transition-transform duration-200 ease-smooth',
    'data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0',
  ),
)
</script>

<template>
  <SwitchRoot
    :model-value="modelValue"
    :disabled="disabled"
    :class="rootClasses"
    :aria-label="ariaLabel"
    @update:model-value="onCheckedChange"
  >
    <SwitchThumb :class="thumbClasses" />
  </SwitchRoot>
</template>
