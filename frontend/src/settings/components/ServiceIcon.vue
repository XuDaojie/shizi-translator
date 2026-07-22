<script setup lang="ts">
import { computed } from 'vue'
import { Icon } from '@iconify/vue'
import type { ServiceId } from '../types'
import {
  getServiceIconifyId,
  getServiceLogoSrc,
  getServiceLucideFallback,
  LUCIDE_CUSTOM_FALLBACK,
} from '../tokens'

const props = defineProps<{
  serviceId: ServiceId
  className?: string
}>()

const iconifyId = computed(() => getServiceIconifyId(props.serviceId))
const logoSrc = computed(() =>
  iconifyId.value ? undefined : getServiceLogoSrc(props.serviceId),
)
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
  <img
    v-else-if="logoSrc"
    :src="logoSrc"
    alt=""
    draggable="false"
    :class="['service-logo', props.className]"
  />
  <component
    :is="Fallback"
    v-else
    :class="props.className"
  />
</template>

<style scoped>
.service-logo {
  display: block;
  object-fit: contain;
  flex-shrink: 0;
}
</style>
