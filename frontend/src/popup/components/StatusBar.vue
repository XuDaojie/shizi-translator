<script setup lang="ts">
interface StatusAction {
  label: string
  onClick: () => void
}

interface Props {
  text: string
  loading: boolean
  action?: StatusAction | null
  charCount: number
}

withDefaults(defineProps<Props>(), { action: null })
</script>

<template>
  <div class="status-bar">
    <div class="status-left">
      <span class="status-dot" :class="{ loading }" />
      <span>{{ text }}</span>
      <button v-if="action" class="status-action" @click="action.onClick">{{ action.label }}</button>
    </div>
    <span>{{ charCount }} 字</span>
  </div>
</template>
