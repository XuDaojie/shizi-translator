<script setup lang="ts">
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';

const props = defineProps<{
  popupPrecreate: boolean;
  overlayPrecreate: boolean;
  collectUsage: boolean;
}>();
const emit = defineEmits<{
  'update:popupPrecreate': [v: boolean];
  'update:overlayPrecreate': [v: boolean];
  'update:collectUsage': [v: boolean];
}>();
</script>

<template>
  <div class="space-y-3">
    <h3 class="text-sm font-medium">窗口策略</h3>
    <div class="flex items-center justify-between">
      <Label>预创建翻译弹窗</Label>
      <Switch :model-value="props.popupPrecreate" @update:model-value="(v: boolean) => emit('update:popupPrecreate', v)" />
    </div>
    <div class="flex items-center justify-between">
      <Label>预创建截图窗口</Label>
      <Switch :model-value="props.overlayPrecreate" @update:model-value="(v: boolean) => emit('update:overlayPrecreate', v)" />
    </div>
    <div class="flex items-center justify-between">
      <Label>采集 token 用量<span class="block text-xs text-muted-foreground font-normal">显示翻译 token 消耗</span></Label>
      <Switch :model-value="props.collectUsage" @update:model-value="(v: boolean) => emit('update:collectUsage', v)" />
    </div>
  </div>
</template>
