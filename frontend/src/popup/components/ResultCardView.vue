<script setup lang="ts">
import ServiceIcon from '@/settings/components/ServiceIcon.vue'

type CardStatus = 'success' | 'loading' | 'error' | 'aborted' | 'pending'

interface Props {
  engineName: string
  /** 服务渠道 type（如 openai / deepseek），与设置页服务列表共用 ServiceIcon */
  serviceType?: string
  modelName?: string
  /** 已完成译文；流式态由默认 slot 提供 */
  text?: string
  status?: CardStatus
  /** 流式加载中（弹窗逐字流式时驱动蓝点 + 光标） */
  loading?: boolean
  collapsed?: boolean
  hasOverflow?: boolean
  expanded?: boolean
  showTokens?: boolean
  inputTokens?: number
  outputTokens?: number
  /** 是否显示底部 actions（朗读 / 复制） */
  showActions?: boolean
  /** 失败/中断时是否在操作栏右侧显示「刷新」按钮 */
  showRefresh?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  serviceType: '',
  modelName: '',
  text: '',
  status: 'success',
  loading: false,
  collapsed: false,
  hasOverflow: false,
  expanded: false,
  showTokens: true,
  inputTokens: 0,
  outputTokens: 0,
  showActions: true,
  showRefresh: false,
})

const emit = defineEmits<{
  (e: 'toggle-collapse'): void
  (e: 'toggle-expand'): void
  (e: 'speak'): void
  (e: 'copy'): void
  (e: 'refresh'): void
}>()

const onHeaderClick = (e: MouseEvent): void => {
  if ((e.target as HTMLElement).closest('.result-collapse-btn')) return
  emit('toggle-collapse')
}
const onCollapseClick = (e: MouseEvent): void => { e.stopPropagation(); emit('toggle-collapse') }
const onExpandClick = (e: MouseEvent): void => { e.stopPropagation(); emit('toggle-expand') }

const dotClass = (): string => {
  if (props.status === 'error' || props.status === 'aborted') return 'result-header-dot is-error'
  return 'result-header-dot'
}
const showDotFinal = (): boolean => props.loading || props.status === 'loading'
</script>

<template>
  <div
    class="result-card"
    :class="{
      'collapsed': collapsed,
      'has-overflow': hasOverflow,
      'expanded': expanded,
      'failed': status === 'error',
      'cancelled': status === 'aborted',
    }"
  >
    <div class="result-card-header" @click="onHeaderClick">
      <span class="result-engine-icon" aria-hidden="true">
        <ServiceIcon
          v-if="serviceType"
          :service-id="serviceType"
          class-name="result-engine-icon-glyph"
        />
        <span v-else class="result-engine-icon-letter">
          {{ (engineName || '?').trim().charAt(0).toUpperCase() || '?' }}
        </span>
      </span>
      <span class="result-engine-name">{{ engineName }}</span>
      <span class="result-header-status" :hidden="!showDotFinal()">
        <span :class="dotClass()" />
      </span>
      <button class="result-collapse-btn" title="折叠" @click="onCollapseClick">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
      </button>
    </div>
    <div class="result-card-body">
      <div class="result-card-body-inner">
        <div class="result-text-clip">
          <slot>
            <div class="result-text">{{ text }}<span v-if="status === 'loading'" class="stream-cursor" /></div>
          </slot>
        </div>
        <button class="result-expand-btn" type="button" tabindex="-1" @click="onExpandClick">
          <span class="result-expand-label">{{ expanded ? '收起' : '展开全文' }}</span>
          <svg class="result-expand-chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
        </button>
        <div class="result-actions" :style="{ visibility: showActions ? 'visible' : 'hidden' }">
          <button class="result-action-btn" title="朗读翻译" @click="emit('speak')">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" /><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07" /></svg>
          </button>
          <button class="result-action-btn" title="复制翻译" @click="emit('copy')">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
          </button>
          <button
            v-if="showRefresh && (status === 'error' || status === 'aborted')"
            class="result-action-btn result-refresh-btn"
            title="重新翻译"
            @click="emit('refresh')"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 0 1 15-6.7L21 8" /><path d="M21 3v5h-5" /><path d="M21 12a9 9 0 0 1-15 6.7L3 16" /><path d="M3 21v-5h5" /></svg>
          </button>
          <span v-if="modelName || showTokens" class="result-model-group">
            <span v-if="modelName" class="result-model-tag">{{ modelName }}</span>
            <span v-if="showTokens" class="result-tokens" title="输入 / 输出 Token">
              <span class="tok"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5" /><polyline points="5 12 12 5 19 12" /></svg>{{ inputTokens }}</span>
              <span class="tok-sep" />
              <span class="tok"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19" /><polyline points="19 12 12 19 5 12" /></svg>{{ outputTokens }}</span>
            </span>
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
