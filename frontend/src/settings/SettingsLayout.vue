<script setup lang="ts">
import { computed } from 'vue'
import { useSettings } from './stores/settings'
import { Check, X } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import SettingsSidebar from './SettingsSidebar.vue'

interface Props {
  active: string
}

const props = defineProps<Props>()

const { state, dirty, save, discard } = useSettings()

const emit = defineEmits<{
  (e: 'update:active', value: string): void
}>()

const lastSaved = computed(() => {
  if (dirty.value) return '有未保存的修改'
  return '所有更改已保存'
})
</script>

<template>
  <div class="flex h-full min-h-0">
    <SettingsSidebar
      :model-value="active"
      @update:model-value="(v) => emit('update:active', v)"
    />

    <main class="flex-1 min-w-0 flex flex-col bg-background">
      <div class="flex-1 overflow-y-auto p-4 scrollbar-thin">
        <div class="mx-auto max-w-[var(--content-max-width)] flex flex-col gap-6">
          <slot :state="state" />
        </div>
      </div>

      <footer
        class="sticky bottom-0 z-10 flex items-center justify-between gap-3 border-t border-border bg-background/85 px-4 py-3 backdrop-blur"
      >
        <div
          :class="[
            'flex items-center gap-2 text-[11px] font-medium',
            dirty ? 'text-amber-600 dark:text-amber-400' : 'text-muted-foreground',
          ]"
        >
          <span
            :class="['h-1.5 w-1.5 rounded-full', dirty ? 'bg-amber-500' : 'bg-emerald-500']"
          />
          {{ lastSaved }}
        </div>
        <div v-if="dirty" class="flex items-center gap-2">
          <Button variant="ghost" size="sm" @click="discard">
            <X class="h-3.5 w-3.5" />
            放弃修改
          </Button>
          <Button size="sm" @click="save">
            <Check class="h-3.5 w-3.5" />
            保存
          </Button>
        </div>
      </footer>
    </main>
  </div>
</template>
