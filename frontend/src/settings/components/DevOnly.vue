<script setup lang="ts">
import { useDevMode } from '../composables/useDevMode'

const isDev = useDevMode()
</script>

<template>
  <template v-if="isDev"><slot /></template>
</template>
