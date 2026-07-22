<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import ResultCardView from './ResultCardView.vue'
import type { CardState } from '../composables/useTranslationEvents'
import { displayModelName, POPUP_MESSAGE_KEYS, resultStatusMeta, shouldShowTokens } from '../composables/resultCardMeta'
import { speakText, copyText, getTauriApis } from '../composables/utils'
import { toast } from '@/lib/toast'
import { t } from '@/i18n'

interface Props {
  card: CardState
  targetLang: string
}

const props = defineProps<Props>()
const emit = defineEmits<{ (e: 'toggle-expand', card: CardState): void }>()

const textRef = ref<HTMLElement | null>(null)

/* ResultCardView 的 status 映射：CardState.status -> 展示态。 */
const viewStatus = computed<'success' | 'loading' | 'pending' | 'error' | 'aborted'>(() => {
  switch (props.card.status) {
    case 'translating': return 'loading'
    case 'finished': return 'success'
    case 'failed': return 'error'
    case 'cancelled': return 'aborted'
    default: return 'pending'
  }
})
const isLoading = computed(() => props.card.status === 'translating')
const statusMeta = computed(() => resultStatusMeta(props.card.status))

/* 流式渲染：watch card.text，增量 appendChild TextNode / 全量 textContent 替换，
   命令式管理光标 span（复刻旧 setStreamCursor + scrollToBottom）。flush:sync 保证不丢帧。 */
const renderText = (newText: string, oldText: string | undefined): void => {
  const el = textRef.value
  if (!el) return
  // 移除旧光标
  el.querySelector('.stream-cursor')?.remove()
  if (oldText !== undefined && newText.startsWith(oldText)) {
    el.appendChild(document.createTextNode(newText.slice(oldText.length)))
  } else {
    el.textContent = newText
  }
  if (props.card.status === 'translating') {
    const cursor = document.createElement('span')
    cursor.className = 'stream-cursor'
    el.appendChild(cursor)
  }
  el.scrollTop = el.scrollHeight
}

watch(() => props.card.text, (newText, oldText) => renderText(newText, oldText), { flush: 'sync' })

/* 挂载后若已有 text（如重试/回填），立即渲染一次。 */
nextTick(() => {
  if (props.card.text && textRef.value && !textRef.value.textContent) {
    renderText(props.card.text, undefined)
  }
})

const onToggleCollapse = (): void => {
  props.card.collapsed = !props.card.collapsed
  props.card.collapseUserOverride = true
}

const onToggleExpand = (): void => {
  props.card.expanded = !props.card.expanded
  emit('toggle-expand', props.card)
}

/* overflow 检测（复刻旧 detectOverflow）：展开按钮可见性。 */
const detectOverflow = (): void => {
  const el = textRef.value?.parentElement /* .result-text-clip */
  if (!el || !textRef.value) return
  props.card.hasOverflow = textRef.value.scrollHeight > el.clientHeight + 1
}
watch(() => props.card.text, () => { nextTick(detectOverflow) })
watch(() => props.card.status, (s) => {
  if (s !== 'translating') {
    textRef.value?.querySelector('.stream-cursor')?.remove()
  }
  if (s === 'finished') nextTick(detectOverflow)
})

const onSpeak = (): void => {
  const text = textRef.value?.textContent ?? props.card.text
  speakText(text, props.targetLang)
}

const onCopy = async (): Promise<void> => {
  const text = textRef.value?.textContent ?? props.card.text
  const ok = await copyText(text)
  if (ok) toast.success(t(POPUP_MESSAGE_KEYS.copySuccess))
  else toast.error(t('popup.error.copyFailed'))
}

const onRefresh = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) { toast.info(t('popup.error.tauriUnavailable')); return }
  try {
    await apis.invoke('retry_translation')
  } catch (e) {
    toast.error(t('popup.error.retryFailed'), String(e))
  }
}
</script>

<template>
  <ResultCardView
    :engine-name="card.serviceName"
    :service-type="card.serviceType"
    :model-name="displayModelName(card.protocol, card.modelName)"
    :text="card.text"
    :status="viewStatus"
    :loading="isLoading"
    :collapsed="card.collapsed"
    :has-overflow="card.hasOverflow"
    :expanded="card.expanded"
    :show-tokens="shouldShowTokens(card.protocol, card.usage !== null)"
    :input-tokens="card.usage?.inputTokens ?? 0"
    :output-tokens="card.usage?.outputTokens ?? 0"
    :show-actions="card.showActions"
    :show-refresh="card.status === 'failed' || card.status === 'cancelled'"
    @toggle-collapse="onToggleCollapse"
    @toggle-expand="onToggleExpand"
    @speak="onSpeak"
    @copy="onCopy"
    @refresh="onRefresh"
  >
    <div ref="textRef" class="result-text" dir="auto" />
    <div v-if="statusMeta && card.status !== 'translating'" class="result-text" dir="auto">
      <strong>{{ t(statusMeta.key, statusMeta.params) }}</strong>
      <span v-if="card.errorMessage">: {{ card.errorMessage }}</span>
    </div>
  </ResultCardView>
</template>
