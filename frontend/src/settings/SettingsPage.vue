<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import SettingsLayout from './SettingsLayout.vue'
import GeneralPanel from './panels/GeneralPanel.vue'
import TranslatePanel from './panels/TranslatePanel.vue'
import ShortcutPanel from './panels/ShortcutPanel.vue'
import ServicesPanel from './panels/ServicesPanel.vue'
import AdvancedPanel from './panels/AdvancedPanel.vue'
import HistoryPanel from './panels/HistoryPanel.vue'
import { useSettings } from './stores/settings'

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
onMounted(() => {
  void settings.syncFromBackend()
})
</script>

<template>
  <SettingsLayout
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
