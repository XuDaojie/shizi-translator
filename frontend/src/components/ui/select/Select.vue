<script setup lang="ts">
import { computed } from 'vue'
import {
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectIcon,
  SelectPortal,
  SelectContent,
  SelectViewport,
  SelectItem,
  SelectItemText,
  SelectItemIndicator,
  type SelectRootProps,
} from 'reka-ui'
import { Check, ChevronDown } from '@lucide/vue'
import { cn } from '@/lib/utils'

interface Option {
  label: string
  value: string
  description?: string
}

interface Props extends Omit<SelectRootProps, 'modelValue' | 'class'> {
  modelValue?: string
  options: Option[]
  placeholder?: string
  disabled?: boolean
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const onUpdate = (value: string): void => {
  emit('update:modelValue', value)
}

const triggerClasses = computed(() =>
  cn(
    'flex h-9 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm',
    'transition-colors duration-150 ease-smooth',
    'focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-1',
    'disabled:cursor-not-allowed disabled:opacity-50',
    'data-[placeholder]:text-muted-foreground',
    props.class,
  ),
)

const contentClasses = cn(
  'relative z-50 max-h-96 min-w-[8rem] overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md',
  'data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2',
)
</script>

<template>
  <SelectRoot :model-value="modelValue" :disabled="disabled" @update:model-value="onUpdate">
    <SelectTrigger :class="triggerClasses">
      <SelectValue :placeholder="placeholder" />
      <SelectIcon class="ml-2 h-4 w-4 opacity-60">
        <ChevronDown />
      </SelectIcon>
    </SelectTrigger>
    <SelectPortal>
      <SelectContent :class="contentClasses" position="popper" :side-offset="4">
        <SelectViewport class="p-1">
          <SelectItem
            v-for="opt in options"
            :key="opt.value"
            :value="opt.value"
            class="relative flex w-full cursor-pointer select-none items-center rounded-sm py-1.5 pl-8 pr-2 text-sm outline-none transition-colors duration-150 focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50"
          >
            <span class="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
              <SelectItemIndicator>
                <Check class="h-4 w-4" />
              </SelectItemIndicator>
            </span>
            <SelectItemText>{{ opt.label }}</SelectItemText>
          </SelectItem>
        </SelectViewport>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>
