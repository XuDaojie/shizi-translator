<script setup lang="ts">
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
</script>

<template>
  <div class="space-y-1.5">
    <Label>目标语言</Label>
    <Input
      :value="props.modelValue"
      placeholder="中文"
      @input="(e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value)"
    />
  </div>
</template>
