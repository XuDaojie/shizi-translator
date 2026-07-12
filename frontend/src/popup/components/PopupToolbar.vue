<script setup lang="ts">
import { getTauriApis } from '../composables/utils'
import { toast } from '@/lib/toast'
import { t } from '@/i18n'

const props = defineProps<{ pinned: boolean }>()
const emit = defineEmits<{ (e: 'update:pinned', value: boolean): void }>()

const togglePin = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) { toast.info(t('popup.error.tauriUnavailable')); return }
  const next = !props.pinned
  try {
    await apis.getCurrentWindow().setAlwaysOnTop(next)
    emit('update:pinned', next)
    toast.info(t(next ? 'popup.toast.pinned' : 'popup.toast.unpinned'))
  } catch (e) {
    toast.error(t('popup.error.pinFailed'), String(e))
  }
}

const triggerOcr = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) { toast.info(t('popup.error.tauriUnavailable')); return }
  try {
    await apis.invoke('trigger_ocr_translation')
  } catch (e) {
    toast.error(t('popup.error.ocrFailed'), String(e))
  }
}

const openSettings = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    await apis.invoke('open_settings')
  } catch (e) {
    toast.error(t('popup.error.openSettings'), String(e))
  }
}
</script>

<template>
  <div class="toolbar" data-tauri-drag-region>
    <div class="toolbar-left">
      <button class="toolbar-btn" type="button" :class="{ active: pinned }" :title="t(pinned ? 'popup.tooltip.unpin' : 'popup.tooltip.pin')" :aria-label="t(pinned ? 'popup.tooltip.unpin' : 'popup.tooltip.pin')" @click="togglePin">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22" /><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z" /></svg>
      </button>
    </div>
    <div class="toolbar-right">
      <button class="toolbar-btn" type="button" :title="t('popup.tooltip.ocr')" :aria-label="t('popup.tooltip.ocr')" @click="triggerOcr">
        <!-- ScanText：与历史面板截图触发图标同源；尺寸由 .toolbar-btn svg (13px / stroke 1.6) 约束 -->
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7V5a2 2 0 0 1 2-2h2" /><path d="M17 3h2a2 2 0 0 1 2 2v2" /><path d="M21 17v2a2 2 0 0 1-2 2h-2" /><path d="M7 21H5a2 2 0 0 1-2-2v-2" /><path d="M7 8h8" /><path d="M7 12h10" /><path d="M7 16h6" /></svg>
      </button>
      <button class="toolbar-btn" type="button" :title="t('popup.tooltip.settings')" :aria-label="t('popup.tooltip.settings')" @click="openSettings">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" /></svg>
      </button>
    </div>
  </div>
</template>
