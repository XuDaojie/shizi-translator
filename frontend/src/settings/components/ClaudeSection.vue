<script setup lang="ts">
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import ApiKeyField from './ApiKeyField.vue';
import type { ClaudeConfig } from '@/types/config';

const props = defineProps<{ modelValue: ClaudeConfig }>();
const emit = defineEmits<{ 'update:modelValue': [value: ClaudeConfig] }>();

function patch<K extends keyof ClaudeConfig>(key: K, value: ClaudeConfig[K]) {
  emit('update:modelValue', { ...props.modelValue, [key]: value });
}
function numPatch(e: Event) {
  patch('timeoutSeconds', Number((e.target as HTMLInputElement).value));
}
</script>

<template>
  <div class="space-y-3">
    <h3 class="text-sm font-medium">Claude</h3>
    <ApiKeyField
      :model-value="props.modelValue.apiKey ?? ''"
      label="API Key"
      placeholder="sk-ant-..."
      @update:model-value="(v) => patch('apiKey', v.trim() || null)"
    />
    <div class="space-y-1.5">
      <Label>Base URL</Label>
      <Input :value="props.modelValue.baseUrl" placeholder="https://api.anthropic.com" @input="(e: Event) => patch('baseUrl', (e.target as HTMLInputElement).value)" />
    </div>
    <div class="space-y-1.5">
      <Label>Model</Label>
      <Input :value="props.modelValue.model" placeholder="claude-haiku-4-5" @input="(e: Event) => patch('model', (e.target as HTMLInputElement).value)" />
    </div>
    <div class="space-y-1.5">
      <Label>Timeout 秒</Label>
      <Input type="number" min="1" step="1" :value="props.modelValue.timeoutSeconds" placeholder="60" @input="numPatch" />
    </div>
    <div class="flex items-center justify-between">
      <Label class="leading-tight">Enable Thinking<br /><span class="text-xs text-muted-foreground font-normal">仅对支持的模型生效，Haiku 需关闭</span></Label>
      <Switch :model-value="props.modelValue.enableThinking" @update:model-value="(v: boolean) => patch('enableThinking', v)" />
    </div>
  </div>
</template>
