<script setup lang="ts">
import { Button } from '@/components/ui/button';

const props = defineProps<{
  saving: boolean;
  status: string;
  isError: boolean;
}>();
const emit = defineEmits<{ save: [] }>();
</script>

<template>
  <div class="space-y-2">
    <Button :disabled="props.saving" @click="emit('save')">
      {{ props.saving ? '保存中...' : '保存配置' }}
    </Button>
    <p class="text-sm" :class="props.isError ? 'text-destructive' : 'text-muted-foreground'">
      {{ props.status }}
    </p>
  </div>
</template>
