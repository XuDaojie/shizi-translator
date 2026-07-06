<script setup lang="ts">
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

</script>

<template>
  <div class="flex h-full min-h-0">
    <SettingsSidebar
      :model-value="active"
      @update:model-value="(v) => emit('update:active', v)"
    />

    <main class="flex-1 min-w-0 flex flex-col bg-background">
      <div class="flex-1 overflow-y-auto p-2.5 scrollbar-thin">
        <div class="mx-auto flex min-h-full max-w-[var(--content-max-width)] flex-col gap-3">
          <slot :state="state" />
        </div>
      </div>
    </main>
  </div>
</template>
