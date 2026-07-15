<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { RotateCcw } from '@lucide/vue'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import {
  isPromptDirty,
  isPromptDefault,
  resetPromptValue,
  shouldShowCharCount,
  shouldShowDefaultPreview,
} from './setting-textarea-logic'
import { t, type MessageKey } from '@/i18n'

/** 任务 9 再落入 locale 文件；此处 cast 避免 MessageKey 严格校验阻塞 typecheck。 */
const promptKey = (key: string): MessageKey => key as MessageKey

interface Props {
  modelValue: string
  /** 卡片标题；有则渲染完整编辑卡片顶栏标题。 */
  title?: string
  /** 标题下的辅助说明。 */
  description?: string
  /** 开发态徽标，与 SettingRow 一致。 */
  status?: 'wip' | 'planned'
  placeholder?: string
  defaultValue?: string
  /** 可点插入的模板变量，如 `{source_lang}`。 */
  variables?: string[]
  /** 最小行数。 */
  minRows?: number
  /** 最大行数，超过后内容可滚动。 */
  maxRows?: number
  disabled?: boolean
  /** dirty 时显示「重置」。 */
  showReset?: boolean
  /** 为 true 时只显示顶栏，隐藏编辑区（如反思关闭）。 */
  collapsed?: boolean
  /** 折叠态提示文案。 */
  collapsedHint?: string
  className?: string
}

const props = withDefaults(defineProps<Props>(), {
  minRows: 3,
  maxRows: 8,
  disabled: false,
  showReset: true,
  collapsed: false,
  variables: () => [],
  className: '',
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'reset'): void
}>()

const textareaRef = ref<HTMLTextAreaElement | null>(null)
const focused = ref(false)

const charCount = computed(() => props.modelValue.length)

const isDirty = computed(() =>
  isPromptDirty({
    modelValue: props.modelValue,
    defaultValue: props.defaultValue,
    showReset: props.showReset,
  }),
)

const isDefault = computed(() =>
  isPromptDefault({
    modelValue: props.modelValue,
    defaultValue: props.defaultValue,
  }),
)

const showDefaultPreview = computed(() =>
  shouldShowDefaultPreview({
    modelValue: props.modelValue,
    defaultValue: props.defaultValue,
    focused: focused.value,
    collapsed: props.collapsed,
  }),
)

const showCharCount = computed(() =>
  shouldShowCharCount({
    collapsed: props.collapsed,
    focused: focused.value,
    dirty: isDirty.value,
    charCount: charCount.value,
  }),
)

const showHeader = computed(
  () => !!(props.title || isDirty.value || showCharCount.value),
)

const lineHeightRem = 1.45
const minHeight = computed(() => `${props.minRows * lineHeightRem + 0.9}rem`)
const maxHeight = computed(() => `${props.maxRows * lineHeightRem + 0.9}rem`)

const statusLabel: Record<NonNullable<Props['status']>, string> = {
  wip: '开发中',
  planned: '规划中',
}

const onInput = (e: Event): void => {
  emit('update:modelValue', (e.target as HTMLTextAreaElement).value)
}

/** 清空以走默认语义，避免把默认文案写入 model 误标「已自定义」。 */
const onReset = (): void => {
  if (props.defaultValue === undefined) return
  emit('update:modelValue', resetPromptValue())
  emit('reset')
}

const focusEditor = async (): Promise<void> => {
  if (props.disabled || props.collapsed) return
  focused.value = true
  await nextTick()
  textareaRef.value?.focus()
}

/** 在光标处插入变量；无 ref 则追加到末尾。 */
const insertVariable = async (token: string): Promise<void> => {
  if (props.disabled || props.collapsed) return
  const el = textareaRef.value
  const value = props.modelValue
  if (!el) {
    emit('update:modelValue', value + token)
    return
  }
  const start = el.selectionStart ?? value.length
  const end = el.selectionEnd ?? value.length
  const next = value.slice(0, start) + token + value.slice(end)
  emit('update:modelValue', next)
  await nextTick()
  const pos = start + token.length
  el.focus()
  el.setSelectionRange(pos, pos)
}
</script>

<template>
  <div
    :class="
      cn(
        'w-full overflow-hidden rounded-lg border border-border bg-muted/25',
        focused && !collapsed && 'border-primary/40 ring-1 ring-primary/20',
        className,
      )
    "
  >
    <!-- 顶栏：标题 / 状态 / 重置 / 字数 / 额外操作（无 title 时仍可因 dirty/字数/slot 出现） -->
    <div
      v-if="showHeader || $slots['header-end']"
      class="flex items-start justify-between gap-2 border-b border-border/70 px-3 py-2"
    >
      <div class="min-w-0 flex-1">
        <div class="flex flex-wrap items-center gap-1.5">
          <span v-if="title" class="text-[13px] font-medium text-foreground">{{ title }}</span>
          <Badge
            v-if="status"
            variant="warning"
            :title="status === 'wip' ? '该功能尚未开发完成,留作后续迭代' : '已规划,暂未排期'"
            class="px-1.5 py-0 text-[10px] font-normal"
          >
            {{ statusLabel[status] }}
          </Badge>
          <span
            v-if="isDirty"
            class="rounded bg-primary/10 px-1.5 py-0 text-[10px] text-primary"
          >
            {{ t(promptKey('settings.prompt.edited')) }}
          </span>
          <span
            v-else-if="title && isDefault"
            class="rounded bg-muted px-1.5 py-0 text-[10px] text-muted-foreground"
          >
            {{ t(promptKey('settings.prompt.default')) }}
          </span>
        </div>
        <p v-if="description" class="mt-0.5 text-[11px] leading-snug text-muted-foreground">
          {{ description }}
        </p>
        <div v-if="variables.length && !collapsed" class="mt-1.5 flex flex-wrap gap-1">
          <button
            v-for="v in variables"
            :key="v"
            type="button"
            class="rounded border border-border bg-background px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
            :disabled="disabled"
            :title="t(promptKey('settings.prompt.insertVariable'), { name: v })"
            @click="insertVariable(v)"
          >
            {{ v }}
          </button>
        </div>
      </div>

      <div class="flex shrink-0 items-center gap-1.5 pt-0.5">
        <slot name="header-end" />
        <span
          v-if="showCharCount"
          class="tabular-nums text-[10px] text-muted-foreground/70"
        >
          {{ t(promptKey('settings.prompt.charCount'), { count: charCount }) }}
        </span>
        <Button
          v-if="isDirty"
          type="button"
          variant="ghost"
          size="icon"
          class="h-7 w-7 text-muted-foreground hover:text-foreground"
          :title="t(promptKey('settings.prompt.reset'))"
          :aria-label="t(promptKey('settings.prompt.reset'))"
          @click="onReset"
        >
          <RotateCcw class="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>

    <!-- 折叠态（如反思关闭） -->
    <p
      v-if="collapsed"
      class="px-3 py-2.5 text-[11px] leading-snug text-muted-foreground"
    >
      {{ collapsedHint || t(promptKey('settings.prompt.collapsed')) }}
    </p>

    <!-- 空态：默认内容预览 -->
    <button
      v-else-if="showDefaultPreview"
      type="button"
      class="block w-full px-3 py-2.5 text-left transition-colors hover:bg-muted/40"
      :disabled="disabled"
      @click="focusEditor"
    >
      <span class="mb-1 block text-[10px] text-muted-foreground/80">
        {{ t(promptKey('settings.prompt.useDefaultHint')) }}
      </span>
      <span class="line-clamp-2 whitespace-pre-wrap font-mono text-[11px] leading-relaxed text-muted-foreground/70">
        {{ defaultValue }}
      </span>
    </button>

    <!-- 编辑区 -->
    <div v-else class="px-2.5 py-2">
      <textarea
        ref="textareaRef"
        :value="modelValue"
        :placeholder="placeholder || ''"
        :disabled="disabled"
        :rows="minRows"
        :class="
          cn(
            'w-full min-w-0 resize-y rounded-md border border-transparent bg-background/80 px-2.5 py-2',
            'font-mono text-xs leading-relaxed text-foreground placeholder:text-muted-foreground/60',
            'transition-colors duration-150',
            'hover:border-border',
            'focus:border-primary/40 focus:outline-none focus:ring-1 focus:ring-primary/25',
            'disabled:cursor-not-allowed disabled:opacity-60',
          )
        "
        :style="{ minHeight, maxHeight }"
        @input="onInput"
        @focus="focused = true"
        @blur="focused = false"
      />
    </div>
  </div>
</template>
