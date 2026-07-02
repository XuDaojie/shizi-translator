<script setup lang="ts">
import { ref } from 'vue';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';

const props = defineProps<{
  modelValue: string;
  label: string;
  placeholder?: string;
}>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();

const show = ref(false);
</script>

<template>
  <div class="space-y-1.5">
    <Label>{{ props.label }}</Label>
    <div class="flex gap-2">
      <Input
        :type="show ? 'text' : 'password'"
        :value="props.modelValue"
        :placeholder="props.placeholder"
        class="flex-1"
        @input="(e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value)"
      />
      <Button variant="outline" size="icon" type="button" @click="show = !show">
        <svg v-if="show" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94"/><path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19"/><path d="m14.12 14.12a3 3 0 1 1-4.24-4.24"/><line x1="1" y1="1" x2="23" y2="23"/></svg>
        <svg v-else xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>
        <span class="sr-only">{{ show ? '隐藏' : '显示' }}</span>
      </Button>
    </div>
  </div>
</template>
