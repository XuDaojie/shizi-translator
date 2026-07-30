<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import ResultCardView from './ResultCardView.vue'
import type { CardState } from '../composables/useTranslationEvents'
import { displayModelName, POPUP_MESSAGE_KEYS, resultStatusMeta, shouldShowTokens } from '../composables/resultCardMeta'
import {
  speakText,
  copyText,
  getTauriApis,
} from '../composables/utils'
import {
  handleMarkdownLinkClick,
  plainTextFromMarkdown,
  renderMarkdownToHtml,
} from '../composables/renderMarkdown'
import { toast } from '@/lib/toast'
import { t } from '@/i18n'

interface Props {
  card: CardState
  targetLang: string
  /** 是否对完成态译文做 Markdown 渲染；默认开启 */
  markdownRender?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  markdownRender: true,
})
const emit = defineEmits<{ (e: 'toggle-expand', card: CardState): void }>()

const textRef = ref<HTMLElement | null>(null)
const mdRef = ref<HTMLElement | null>(null)

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
/** 流式阶段纯文本；完成后按配置决定是否 Markdown HTML */
const showMarkdown = computed(
  () =>
    props.markdownRender
    && props.card.status === 'finished'
    && Boolean(props.card.text.trim()),
)
const markdownHtml = computed(() =>
  showMarkdown.value ? renderMarkdownToHtml(props.card.text) : '',
)

/* 流式渲染：watch card.text，增量 appendChild TextNode / 全量 textContent 替换，
   命令式管理光标 span（复刻旧 setStreamCursor + scrollToBottom）。flush:sync 保证不丢帧。 */
const renderText = (newText: string, oldText: string | undefined): void => {
  if (!isLoading.value) return
  const el = textRef.value
  if (!el) return
  el.querySelector('.stream-cursor')?.remove()
  if (oldText !== undefined && newText.startsWith(oldText)) {
    el.appendChild(document.createTextNode(newText.slice(oldText.length)))
  } else {
    el.textContent = newText
  }
  const cursor = document.createElement('span')
  cursor.className = 'stream-cursor'
  el.appendChild(cursor)
  el.scrollTop = el.scrollHeight
}

watch(() => props.card.text, (newText, oldText) => renderText(newText, oldText), { flush: 'sync' })

/* 进入流式时挂载层后补渲染 */
watch(isLoading, (loading) => {
  if (loading) {
    nextTick(() => renderText(props.card.text, undefined))
  }
})

nextTick(() => {
  if (isLoading.value && props.card.text && textRef.value) {
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

const activeTextEl = (): HTMLElement | null => mdRef.value ?? textRef.value

/* overflow 检测（复刻旧 detectOverflow）：展开按钮可见性。 */
const detectOverflow = (): void => {
  const textEl = activeTextEl()
  const clip = textEl?.parentElement
  if (!clip || !textEl) return
  props.card.hasOverflow = textEl.scrollHeight > clip.clientHeight + 1
}
watch(() => props.card.text, () => { nextTick(detectOverflow) })
watch(() => props.card.status, (s) => {
  if (s === 'finished') nextTick(detectOverflow)
})
watch(markdownHtml, () => { nextTick(detectOverflow) })

const onSpeak = (): void => {
  const text = props.markdownRender
    ? (plainTextFromMarkdown(props.card.text) || props.card.text)
    : props.card.text
  speakText(text, props.targetLang)
}

const onCopy = async (): Promise<void> => {
  // 复制 Markdown 源文，便于二次编辑
  const ok = await copyText(props.card.text)
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

const openMarkdownUrl = async (url: string): Promise<void> => {
  const apis = getTauriApis()
  if (apis) {
    try {
      await apis.invoke('open_url', { url })
      return
    } catch {
      /* 降级 */
    }
  }
  window.open(url, '_blank', 'noopener,noreferrer')
}

const onMarkdownClick = (e: MouseEvent): void => {
  void handleMarkdownLinkClick(e, openMarkdownUrl)
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
    <!-- 流式：纯文本 + 光标 -->
    <div
      v-if="isLoading"
      ref="textRef"
      class="result-text"
      dir="auto"
    />
    <!-- 完成：Markdown 安全 HTML（markdownRender 开启时） -->
    <div
      v-else-if="showMarkdown"
      ref="mdRef"
      class="result-text result-md"
      dir="auto"
      @click="onMarkdownClick"
      v-html="markdownHtml"
    />
    <!-- 完成但关闭 Markdown，或失败/取消时保留片段：纯文本 -->
    <div
      v-else-if="card.text"
      class="result-text"
      dir="auto"
    >{{ card.text }}</div>
    <div v-if="statusMeta && card.status !== 'translating'" class="result-text" dir="auto">
      <strong>{{ t(statusMeta.key, statusMeta.params) }}</strong>
      <span v-if="card.errorMessage">: {{ card.errorMessage }}</span>
    </div>
  </ResultCardView>
</template>
