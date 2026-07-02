<script setup lang="ts">
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import ApiKeyField from './ApiKeyField.vue';
import type { OpenAiCompatibleConfig } from '@/types/config';

const props = defineProps<{ modelValue: OpenAiCompatibleConfig }>();
const emit = defineEmits<{ 'update:modelValue': [value: OpenAiCompatibleConfig] }>();

function patch<K extends keyof OpenAiCompatibleConfig>(key: K, value: OpenAiCompatibleConfig[K]) {
  emit('update:modelValue', { ...props.modelValue, [key]: value });
}
function numPatch(e: Event) {
  patch('timeoutSeconds', Number((e.target as HTMLInputElement).value));
}
</script>

<template>
  <div class="space-y-3">
    <h3 class="text-sm font-medium">OpenAI Compatible</h3>
    <ApiKeyField
      :model-value="props.modelValue.apiKey ?? ''"
      label="API Key"
      placeholder="sk-..."
      @update:model-value="(v) => patch('apiKey', v.trim() || null)"
    />
    <div class="space-y-1.5">
      <Label>Base URL</Label>
      <Input :value="props.modelValue.baseUrl" placeholder="https://api.openai.com/v1" @input="(e: Event) => patch('baseUrl', (e.target as HTMLInputElement).value)" />
    </div>
    <div class="space-y-1.5">
      <Label>Model</Label>
      <Input :value="props.modelValue.model" placeholder="gpt-4o-mini" @input="(e: Event) => patch('model', (e.target as HTMLInputElement).value)" />
    </div>
    <div class="space-y-1.5">
      <Label>Timeout 秒</Label>
      <Input type="number" min="1" step="1" :value="props.modelValue.timeoutSeconds" placeholder="60" @input="numPatch" />
    </div>
  </div>
</template>
