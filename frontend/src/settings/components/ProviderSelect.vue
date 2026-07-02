<script setup lang="ts">
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import type { Provider } from '@/types/config';

const props = defineProps<{ modelValue: Provider }>();
const emit = defineEmits<{ 'update:modelValue': [value: Provider] }>();
</script>

<template>
  <div class="space-y-1.5">
    <Label>Provider</Label>
    <Select :model-value="props.modelValue" @update:model-value="(v: any) => emit('update:modelValue', v as Provider)">
      <SelectTrigger>
        <SelectValue placeholder="选择 Provider" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="openai-compatible">OpenAI Compatible</SelectItem>
        <SelectItem value="claude">Claude</SelectItem>
        <SelectItem value="mock">Mock（调试用）</SelectItem>
      </SelectContent>
    </Select>
  </div>
</template>
