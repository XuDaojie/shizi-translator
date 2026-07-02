<script setup lang="ts">
import { computed } from 'vue'
import { Icon } from '@iconify/vue'
import type { ServiceId } from '../types'
import {
  getServiceIconifyId,
  getServiceLucideFallback,
  LUCIDE_CUSTOM_FALLBACK,
} from '../tokens'

const props = defineProps<{
  serviceId: ServiceId
  className?: string
}>()

const iconifyId = computed(() => getServiceIconifyId(props.serviceId))
const isCustom = computed(() => props.serviceId.startsWith('custom_'))
const Fallback = computed(() =>
  isCustom.value ? LUCIDE_CUSTOM_FALLBACK : getServiceLucideFallback(props.serviceId),
)
</script>

<template>
  <Icon
    v-if="iconifyId"
    :icon="iconifyId"
    :class="props.className"
  />
  <component
    :is="Fallback"
    v-else
    :class="props.className"
  />
</template>
