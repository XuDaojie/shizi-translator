<script setup lang="ts">
import { computed } from 'vue'
import { useSettings } from './stores/settings'
import SettingsSidebar from './SettingsSidebar.vue'

interface Props {
  active: string
}

const props = defineProps<Props>()

const { state } = useSettings()

const emit = defineEmits<{
  (e: 'update:active', value: string): void
}>()

/** 服务页自管左右分栏滚动,外层不再出滚动条。 */
const isServices = computed(() => props.active === 'services')
</script>

<template>
  <div class="flex h-full min-h-0">
    <SettingsSidebar
      :model-value="active"
      @update:model-value="(v) => emit('update:active', v)"
    />

    <main class="flex min-h-0 min-w-0 flex-1 flex-col bg-background">
      <div
        :class="[
          'min-h-0 flex-1 p-2.5 scrollbar-thin',
          isServices ? 'flex flex-col overflow-hidden' : 'overflow-y-auto',
        ]"
      >
        <div
          :class="[
            'mx-auto flex max-w-[var(--content-max-width)] flex-col',
            isServices
              ? 'h-full min-h-0 w-full flex-1 overflow-hidden'
              : 'min-h-full gap-3',
          ]"
        >
          <slot :state="state" />
        </div>
      </div>
    </main>
  </div>
</template>
