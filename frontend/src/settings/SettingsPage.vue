<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import SettingsLayout from './SettingsLayout.vue'
import GeneralPanel from './panels/GeneralPanel.vue'
import TranslatePanel from './panels/TranslatePanel.vue'
import ShortcutPanel from './panels/ShortcutPanel.vue'
import ServicesPanel from './panels/ServicesPanel.vue'
import AdvancedPanel from './panels/AdvancedPanel.vue'
import HistoryPanel from './panels/HistoryPanel.vue'
import { useSettings } from './stores/settings'
import { matchShortcutKeys } from '@/lib/matchShortcut'
import { invokeOpenSettings } from '@/lib/tauri'

interface Props {
  initialCategory?: string
}

const props = withDefaults(defineProps<Props>(), {
  initialCategory: 'general',
})

const active = ref<string>(props.initialCategory)
watch(
  () => props.initialCategory,
  (value) => {
    if (value) active.value = value
  },
)

const onUpdateActive = (value: string): void => {
  active.value = value
  if (typeof window !== 'undefined') {
    const url = new URL(window.location.href)
    url.hash = value
    window.history.replaceState({}, '', url)
  }
}

const settings = useSettings()

const openSettingsKeys = computed(
  () => settings.state.shortcut.bindings.find((b) => b.id === 'open-settings')?.keys ?? 'Ctrl+,',
)

const onAppShortcutKeydown = (e: KeyboardEvent): void => {
  // ShortcutRecorder 录入时会 capture + stopPropagation，此处不会收到
  if (!matchShortcutKeys(openSettingsKeys.value, e)) return
  e.preventDefault()
  void invokeOpenSettings().catch(() => {
    // best-effort：设置窗已打开时再触发仅聚焦
  })
}

onMounted(() => {
  void settings.syncFromBackend()
  window.addEventListener('keydown', onAppShortcutKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onAppShortcutKeydown)
})
</script>

<template>
  <SettingsLayout
    class="h-full min-h-0"
    :active="active"
    @update:active="onUpdateActive"
  >
    <template #default="{ state }">
      <GeneralPanel v-if="active === 'general'" :state="state" />
      <TranslatePanel v-else-if="active === 'translate'" :state="state" />
      <ShortcutPanel v-else-if="active === 'shortcut'" :state="state" />
      <ServicesPanel v-else-if="active === 'services'" :state="state" />
      <HistoryPanel v-else-if="active === 'history'" :state="state" />
      <AdvancedPanel v-else :state="state" />
    </template>
  </SettingsLayout>
</template>
