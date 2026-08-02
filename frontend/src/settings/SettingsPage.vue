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
import { locale, reloadCurrentLocale, t } from '@/i18n'
import { createLogger } from '@public/logger.js'
import { getTauriApis } from '@/popup/composables/utils'

interface Props {
  initialCategory?: string
}

const props = withDefaults(defineProps<Props>(), {
  initialCategory: 'general',
})

/** 侧栏高亮（立即更新，驱动指示器动画）。 */
const active = ref<string>(props.initialCategory)
/**
 * 主区实际渲染的分类。历史首屏较重（列表 + ResultCard Markdown），
 * 首次切入时略晚于侧栏，避免与 280ms 导航指示器动画抢主线程。
 */
const contentActive = ref<string>(props.initialCategory)
/** 历史是否已挂载过（再次进入可立即切换，配合 keep-alive）。 */
let historyMountedOnce = props.initialCategory === 'history'
let contentSwitchSeq = 0
/** 与 SettingsSidebar 指示器 transition 对齐，略短以免内容空窗过长。 */
const HISTORY_NAV_SETTLE_MS = 240

watch(
  () => props.initialCategory,
  (value) => {
    if (!value) return
    active.value = value
    contentActive.value = value
    if (value === 'history') historyMountedOnce = true
  },
)

const onUpdateActive = (value: string): void => {
  active.value = value
  if (typeof window !== 'undefined') {
    const url = new URL(window.location.href)
    url.hash = value
    window.history.replaceState({}, '', url)
  }

  const seq = ++contentSwitchSeq
  if (value === 'history' && !historyMountedOnce) {
    window.setTimeout(() => {
      if (seq !== contentSwitchSeq) return
      contentActive.value = value
      historyMountedOnce = true
    }, HISTORY_NAV_SETTLE_MS)
    return
  }
  contentActive.value = value
}

const settings = useSettings()
const logger = createLogger('settings')
let disposed = false
let unlistenLanguageChanged: (() => void) | null = null

const applyDocumentLanguageAndTitle = async (): Promise<void> => {
  document.documentElement.lang = locale.value
  const apis = getTauriApis()
  if (!apis) return
  try {
    await (apis.getCurrentWindow() as ReturnType<typeof apis.getCurrentWindow> & {
      setTitle: (title: string) => Promise<void>
    }).setTitle(t('window.settingsTitle'))
  } catch (error) {
    logger.warn('更新设置窗口标题失败', String(error))
  }
}

const reloadAndApplyLanguage = async (): Promise<void> => {
  try {
    await reloadCurrentLocale()
    if (!disposed) await applyDocumentLanguageAndTitle()
  } catch (error) {
    logger.warn('刷新界面语言失败', String(error))
  }
}

const setupLanguageSync = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) {
    await applyDocumentLanguageAndTitle()
    return
  }
  try {
    const unlisten = await apis.listen('interface-language:changed', () => {
      void reloadAndApplyLanguage()
    })
    if (disposed) {
      unlisten()
      return
    }
    unlistenLanguageChanged = unlisten
    await reloadAndApplyLanguage()
  } catch (error) {
    logger.warn('监听界面语言变更失败', String(error))
  }
}

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
  void setupLanguageSync()
  void settings.syncFromBackend()
  window.addEventListener('keydown', onAppShortcutKeydown)
})

onBeforeUnmount(() => {
  disposed = true
  unlistenLanguageChanged?.()
  window.removeEventListener('keydown', onAppShortcutKeydown)
})
</script>

<template>
  <SettingsLayout
    class="h-full min-h-0"
    :active="active"
    :content-active="contentActive"
    @update:active="onUpdateActive"
  >
    <template #default="{ state }">
      <GeneralPanel v-if="contentActive === 'general'" :state="state" />
      <TranslatePanel v-else-if="contentActive === 'translate'" :state="state" />
      <ShortcutPanel v-else-if="contentActive === 'shortcut'" :state="state" />
      <ServicesPanel v-else-if="contentActive === 'services'" :state="state" />
      <AdvancedPanel v-else-if="contentActive === 'advanced'" :state="state" />
      <!-- keep-alive 须常挂：二次进入历史免整树重挂载；勿接入上方 v-else 链 -->
      <keep-alive>
        <HistoryPanel v-if="contentActive === 'history'" :state="state" />
      </keep-alive>
    </template>
  </SettingsLayout>
</template>
