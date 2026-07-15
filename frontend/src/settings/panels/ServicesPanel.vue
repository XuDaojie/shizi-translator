<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { toast } from '@/lib/toast'
import {
  Plus,
  Trash2,
  Search,
  CircleAlert,
  Pencil,
  Check,
  X,
  Sparkles,
  ScanText,
  Lock,
  BookOpen,
  ExternalLink,
  KeyRound,
  ChevronDown,
} from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Dialog } from '@/components/ui/dialog'
import {
  ApiKeyInput,
  ServiceIcon,
  SettingGroup,
  SettingRow,
  SettingSwitch,
  SettingInput,
  SettingSelect,
  SettingTextarea,
  ModelCombobox,
  DevOnly,
} from '../components'
import type { AppSettings, ServiceId, ServiceInstance, OcrServiceId, OcrServiceInstance } from '../types'
import {
  DEFAULT_OCR_PROMPT,
  DEFAULT_PROMPTS,
  OCR_PICKER_SERVICES,
  ocrServiceById,
  serviceById,
} from '../tokens'
import { isPromptDefault } from '../components/setting-textarea-logic'
import {
  invokeValidateServiceCredential,
  invokeListServiceModels,
  invokeOpenUrl,
  isTauriReady,
} from '@/lib/tauri'
import { useSettings } from '../stores/settings'
import { useDevMode } from '../composables/useDevMode'
import { validateServiceForEnable } from '@/settings/service-validation'
import { t, type MessageKey } from '@/i18n'

/** 任务 9 再落入 locale 文件；此处 cast 避免 MessageKey 严格校验阻塞 typecheck。 */
const msgKey = (key: string): MessageKey => key as MessageKey

const props = defineProps<{
  state: AppSettings
}>()

const settings = useSettings()
const isDev = useDevMode()

/** 思维链长度档位,默认「关闭」对应 `chainOfThought: 'off'`。 */
const chainOfThoughtOptions = computed(() => [
  { label: t('settings.option.off'), value: 'off', description: t('settings.description.reasoningOff') },
  { label: t('settings.option.short'), value: 'short', description: t('settings.description.reasoningShort') },
  { label: t('settings.option.medium'), value: 'medium', description: t('settings.description.reasoningMedium') },
  { label: t('settings.option.long'), value: 'long', description: t('settings.description.reasoningLong') },
])

/** 渠道 protocols 为空即视为"尚未对接"，在 UI 上标记开发中并置灰启用。 */
const isDeveloping = (type: ServiceId): boolean =>
  serviceById(type)?.protocols.length === 0

/** 当前环境下可见的实例：dev 全可见；release 过滤掉未对接渠道实例。 */
const isVisibleInstance = (inst: ServiceInstance): boolean =>
  isDev || !isDeveloping(inst.type)

/** 第一个可见实例 id，无可见实例时返回空字符串（详情页走空态分支）。 */
const firstVisibleInstanceId = (): string =>
  props.state.services.find(isVisibleInstance)?.id ?? ''

const activeInstanceId = ref<string>(firstVisibleInstanceId())
const search = ref('')
const pickerOpen = ref(false)
/** OCR 左列表当前选中实例。 */
const activeOcrInstanceId = ref(props.state.ocrServices[0]?.id ?? '')
const ocrPickerOpen = ref(false)
/** 正在拉取模型的实例 id；用单值 ref 保证模板 loading 能可靠刷新。 */
const pullingId = ref<string | null>(null)
const keyStatusById = ref<Record<string, ServiceInstance['keyStatus']>>({})
const tab = ref<'translate' | 'ocr'>('translate')

// 内置 + 用户自定义渠道的合并视图
const mergedServices = computed(() => settings.getMergedServices())
/** 添加渠道对话框候选：dev 全展示；release 过滤掉未对接渠道（protocols 为空）。 */
const addableServices = computed(() =>
  isDev ? mergedServices.value : mergedServices.value.filter((s) => s.protocols.length > 0),
)

const probeRequest = (inst: ServiceInstance) => ({
  protocol: inst.protocol,
  endpoint: inst.endpoint,
  apiKey: inst.apiKey.trim() || null,
})

// 实例名称行内编辑
const editingName = ref(false)
const nameDraft = ref('')
const nameInput = ref<HTMLInputElement | null>(null)

/** 高级提示词区折叠状态；切换实例时重置为收起。 */
const advancedOpen = ref(false)
/** OCR 视觉详情「识别提示词」折叠；切换 OCR 实例时重置。 */
const advancedOcrOpen = ref(false)

const activeInstance = computed<ServiceInstance | undefined>(() =>
  props.state.services.find((s) => s.id === activeInstanceId.value),
)
const activeService = computed(() =>
  activeInstance.value ? serviceById(activeInstance.value.type) : undefined,
)

const activeOcrInstance = computed(() =>
  props.state.ocrServices.find((s) => s.id === activeOcrInstanceId.value),
)
const activeOcrService = computed(() =>
  activeOcrInstance.value ? ocrServiceById(activeOcrInstance.value.type) : undefined,
)

watch(
  () => props.state.ocrServices.map((s) => s.id).join(','),
  () => {
    if (!props.state.ocrServices.some((s) => s.id === activeOcrInstanceId.value)) {
      activeOcrInstanceId.value = props.state.ocrServices[0]?.id ?? ''
    }
  },
)

const ocrSubtitle = (inst: OcrServiceInstance): string => {
  const meta = ocrServiceById(inst.type)
  if (meta?.detailKind === 'system') return t(msgKey('settings.ocr.systemSubtitle'))
  return t(msgKey('settings.ocr.visionSubtitle'), { model: inst.model || '—' })
}

const onAddOcrService = (type: OcrServiceId): void => {
  const inst = settings.addOcrService(type)
  activeOcrInstanceId.value = inst.id
  ocrPickerOpen.value = false
}

const onOcrSelect = (id: string): void => {
  activeOcrInstanceId.value = id
}

const onOcrToggle = (inst: OcrServiceInstance, enabled: boolean): void => {
  settings.setOcrEnabled(inst.id, enabled)
}

const probeOcrRequest = (inst: OcrServiceInstance) => {
  const meta = ocrServiceById(inst.type)
  return {
    protocol: meta?.protocolId ?? 'openai_chat',
    endpoint: inst.endpoint,
    apiKey: inst.apiKey.trim() || null,
  }
}

/** OCR 模型下拉 = 已拉取 ∪ 内置 models（去重，拉取在前）。 */
const ocrModelOptions = computed((): string[] => {
  const inst = activeOcrInstance.value
  if (!inst) return []
  const builtin = ocrServiceById(inst.type)?.models ?? []
  const seen = new Set<string>()
  const out: string[] = []
  for (const m of [...inst.pulledModels, ...builtin]) {
    const id = m.trim()
    if (!id || seen.has(id)) continue
    seen.add(id)
    out.push(id)
  }
  return out
})

const onOcrKeyValidate = async (key: string): Promise<void> => {
  const inst = activeOcrInstance.value
  if (!inst) return
  if (!key.trim()) {
    inst.keyStatus = 'invalid'
    toast.error(t('settings.toast.validationFailed'), t('settings.toast.apiKeyRequired'))
    return
  }
  if (inst.keyStatus === 'validating') return
  inst.keyStatus = 'validating'
  try {
    await invokeValidateServiceCredential(probeOcrRequest(inst))
    if (activeOcrInstance.value?.id !== inst.id) return
    inst.keyStatus = 'valid'
  } catch (err) {
    if (activeOcrInstance.value?.id !== inst.id) return
    inst.keyStatus = 'invalid'
    toast.error(t('settings.toast.validationFailed'), String(err))
  }
}

const onOcrPullModels = async (instanceId: string): Promise<void> => {
  if (pullingId.value === instanceId) return
  const inst = props.state.ocrServices.find((s) => s.id === instanceId)
  if (!inst) return

  const meta = ocrServiceById(inst.type)
  if (!meta?.hasModelApi) return
  if (inst.pulledModels.length > 0) return

  if (!inst.apiKey.trim()) {
    toast.error(t('settings.toast.pullFailed'), t('settings.toast.apiKeyRequired'))
    return
  }
  if (!inst.endpoint.trim()) {
    toast.error(t('settings.toast.pullFailed'), t('settings.toast.endpointRequired'))
    return
  }

  pullingId.value = instanceId
  await nextTick()
  try {
    const result = await invokeListServiceModels(probeOcrRequest(inst))
    if (activeOcrInstance.value?.id !== inst.id) return
    inst.pulledModels = result.models
    if (result.models.length === 0) {
      toast.info(t('settings.toast.emptyModels'), t('settings.toast.emptyModelsDescription'))
    }
  } catch (err) {
    if (activeOcrInstance.value?.id !== inst.id) return
    toast.error(t('settings.toast.pullFailed'), String(err))
  } finally {
    if (pullingId.value === instanceId) pullingId.value = null
  }
}

const onOcrRemove = (): void => {
  const inst = activeOcrInstance.value
  if (!inst) return
  if (activeOcrService.value?.canDelete === false) return
  if (!window.confirm(t('settings.dialog.deleteService', { name: inst.name }))) return
  settings.removeOcrService(inst.id)
  activeOcrInstanceId.value = props.state.ocrServices[0]?.id ?? ''
}

/** 高级区折叠摘要：默认/自定义提示词 · 反思开启。 */
const advancedSummary = computed(() => {
  const inst = activeInstance.value
  if (!inst) return ''
  const custom =
    !isPromptDefault({ modelValue: inst.systemPrompt, defaultValue: DEFAULT_PROMPTS.system }) ||
    !isPromptDefault({ modelValue: inst.translationPrompt, defaultValue: DEFAULT_PROMPTS.translation }) ||
    !isPromptDefault({ modelValue: inst.reflectionPrompt, defaultValue: DEFAULT_PROMPTS.reflection })
  const parts: string[] = []
  parts.push(custom ? t(msgKey('settings.prompt.summaryCustom')) : t(msgKey('settings.prompt.summaryDefault')))
  if (inst.reflectionEnabled) parts.push(t(msgKey('settings.prompt.summaryReflectionOn')))
  return parts.join(' · ')
})

const openExternal = async (url: string): Promise<void> => {
  try {
    if (isTauriReady()) await invokeOpenUrl(url)
    else window.open(url, '_blank', 'noopener,noreferrer')
  } catch (err) {
    toast.error(t(msgKey('settings.toast.openUrlFailed')), String(err))
  }
}

watch(activeInstanceId, () => {
  advancedOpen.value = false
  editingName.value = false
})

watch(activeOcrInstanceId, () => {
  advancedOcrOpen.value = false
  editingName.value = false
})

/** tab 切换时收起编辑/折叠，避免翻译与 OCR 详情状态串扰。 */
watch(tab, () => {
  editingName.value = false
  advancedOpen.value = false
  advancedOcrOpen.value = false
})

const keyStatusFor = (instanceId: string): ServiceInstance['keyStatus'] =>
  keyStatusById.value[instanceId] ?? 'idle'

const setKeyStatus = (instanceId: string, status: ServiceInstance['keyStatus']): void => {
  keyStatusById.value[instanceId] = status
}

const filteredInstances = computed(() => {
  const q = search.value.trim().toLowerCase()
  // release 包隐藏未对接渠道实例（dev 全可见）
  const list = props.state.services.filter(isVisibleInstance)
  if (!q) return list
  return list.filter((inst) => {
    const meta = serviceById(inst.type)
    return (
      inst.name.toLowerCase().includes(q) ||
      meta?.name.toLowerCase().includes(q) ||
      meta?.description.toLowerCase().includes(q)
    )
  })
})

watch(
  () =>
    tab.value === 'ocr' ? activeOcrInstance.value?.name : activeInstance.value?.name,
  (n) => {
    if (!editingName.value) nameDraft.value = n ?? ''
  },
  { immediate: true },
)

const onServiceSelect = (id: string): void => {
  activeInstanceId.value = id
  editingName.value = false
}

const onKeyValidate = async (key: string): Promise<void> => {
  const inst = activeInstance.value
  if (!inst) return
  if (!key.trim()) {
    setKeyStatus(inst.id, 'invalid')
    toast.error(t('settings.toast.validationFailed'), t('settings.toast.apiKeyRequired'))
    return
  }
  if (keyStatusFor(inst.id) === 'validating') return
  setKeyStatus(inst.id, 'validating')
  try {
    await invokeValidateServiceCredential(probeRequest(inst))
    if (activeInstance.value?.id !== inst.id) return
    setKeyStatus(inst.id, 'valid')
  } catch (err) {
    if (activeInstance.value?.id !== inst.id) return
    setKeyStatus(inst.id, 'invalid')
    toast.error(t('settings.toast.validationFailed'), String(err))
  }
}

const onRemove = (): void => {
  if (!activeInstance.value) return
  const removedId = activeInstance.value.id
  const name = activeInstance.value.name
  if (!window.confirm(t('settings.dialog.deleteService', { name }))) return
  settings.removeService(removedId)
  activeInstanceId.value = firstVisibleInstanceId()
}

const onAddService = (type: ServiceId): void => {
  const inst = settings.addService(type)
  activeInstanceId.value = inst.id
  pickerOpen.value = false
}

/** 下拉候选 = 内置静态 models ∪ 已拉取列表（去重，拉取结果在前便于发现新模型）。 */
const modelOptions = computed((): string[] => {
  const inst = activeInstance.value
  if (!inst) return []
  const builtin = serviceById(inst.type)?.models ?? []
  const seen = new Set<string>()
  const out: string[] = []
  for (const m of [...inst.pulledModels, ...builtin]) {
    const id = m.trim()
    if (!id || seen.has(id)) continue
    seen.add(id)
    out.push(id)
  }
  return out
})

/**
 * 打开模型下拉时拉取 OpenAI 兼容 `GET {endpoint}/models`（DeepSeek/智谱/Moonshot 等同一路径）。
 * - 已成功拉取过（pulledModels 非空）则跳过，避免每次打开都打接口
 * - 缺 Key / Endpoint 时先 toast，并仍显示 loading 一瞬，避免「点了没反应」
 */
const onPullModels = async (instanceId: string): Promise<void> => {
  if (pullingId.value === instanceId) return
  const inst = props.state.services.find((s) => s.id === instanceId)
  if (!inst) return

  const meta = serviceById(inst.type)
  if (!meta?.hasModelApi) return
  // 已有拉取结果：不重复请求（用户改 Key/Endpoint 后可清空 pulledModels 或删实例重建）
  if (inst.pulledModels.length > 0) return

  if (!inst.apiKey.trim()) {
    toast.error(t('settings.toast.pullFailed'), t('settings.toast.apiKeyRequired'))
    return
  }
  if (!inst.endpoint.trim()) {
    toast.error(t('settings.toast.pullFailed'), t('settings.toast.endpointRequired'))
    return
  }

  pullingId.value = instanceId
  // 让出一帧，确保 ModelCombobox 的 loading 转圈 / 底栏能先画出来
  await nextTick()
  try {
    const result = await invokeListServiceModels(probeRequest(inst))
    if (activeInstance.value?.id !== inst.id) return
    inst.pulledModels = result.models
    if (result.models.length === 0) {
      toast.info(t('settings.toast.emptyModels'), t('settings.toast.emptyModelsDescription'))
    }
  } catch (err) {
    if (activeInstance.value?.id !== inst.id) return
    toast.error(t('settings.toast.pullFailed'), String(err))
  } finally {
    if (pullingId.value === instanceId) pullingId.value = null
  }
}

const enterNameEdit = async (): Promise<void> => {
  if (tab.value === 'ocr') {
    const inst = activeOcrInstance.value
    // system（Windows）不可重命名
    if (!inst || activeOcrService.value?.detailKind === 'system') return
    nameDraft.value = inst.name
  } else {
    if (!activeInstance.value) return
    nameDraft.value = activeInstance.value.name
  }
  editingName.value = true
  await nextTick()
  nameInput.value?.focus()
  nameInput.value?.select()
}

const commitNameEdit = (): void => {
  if (!editingName.value) return
  const next = nameDraft.value.trim()
  if (tab.value === 'ocr') {
    const inst = activeOcrInstance.value
    if (inst && next && next !== inst.name) {
      settings.renameOcrService(inst.id, next)
    }
    editingName.value = false
    return
  }
  if (!activeInstance.value) {
    editingName.value = false
    return
  }
  if (next && next !== activeInstance.value.name) {
    activeInstance.value.name = next
  }
  editingName.value = false
}

const cancelNameEdit = (): void => {
  editingName.value = false
  nameDraft.value =
    tab.value === 'ocr'
      ? (activeOcrInstance.value?.name ?? '')
      : (activeInstance.value?.name ?? '')
}

/** 拖拽重排:在 services 数组里拖动实例改变顺序。 */
const draggedId = ref<string | null>(null)
const dropTargetId = ref<string | null>(null)
const dropPosition = ref<'before' | 'after'>('before')

function handleToggle(instance: ServiceInstance): void {
  if (!instance.enabled) {
    const err = validateServiceForEnable(instance, serviceById(instance.type))
    if (err) {
    toast.error(t('settings.toast.enableFailed'), err)
      return
    }
  }
  instance.enabled = !instance.enabled
}

const onDragStart = (e: DragEvent, id: string): void => {
  if (!e.dataTransfer) return
  draggedId.value = id
  e.dataTransfer.effectAllowed = 'move'
  e.dataTransfer.setData('text/plain', id)
}

const onDragOver = (e: DragEvent, targetId: string): void => {
  if (!draggedId.value || targetId === draggedId.value) return
  e.preventDefault()
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
  dropTargetId.value = targetId
  // 根据光标在 target 行内的相对位置决定 before / after
  const el = e.currentTarget as HTMLElement | null
  if (!el) return
  const rect = el.getBoundingClientRect()
  dropPosition.value = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after'
}

const onDrop = (e: DragEvent, targetId: string): void => {
  e.preventDefault()
  const fromId = draggedId.value
  draggedId.value = null
  dropTargetId.value = null
  if (!fromId || fromId === targetId) return
  settings.reorderService(fromId, targetId, dropPosition.value)
}

const onDragEnd = (): void => {
  draggedId.value = null
  dropTargetId.value = null
}
</script>

<template>
  <!-- 铺满内容区高度:左侧列表固定高度内部滚,「添加服务」钉在底部;右侧详情独立滚 -->
  <div class="grid h-full min-h-0 grid-cols-[220px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)] gap-2.5 overflow-hidden">
    <!-- 左侧:服务实例列表(固定高度,内部独立滚动,不随外层/右侧滚动) -->
    <aside class="flex h-full min-h-0 flex-col gap-2.5 overflow-hidden self-stretch">
      <!-- Tab 栏:翻译服务 / 文字识别 -->
      <div class="flex shrink-0 items-center gap-1 border-b border-border">
        <button
          type="button"
          :class="[
            'relative px-2.5 pb-1.5 pt-0.5 text-xs font-medium transition-colors',
            tab === 'translate'
              ? 'text-foreground'
              : 'text-muted-foreground hover:text-foreground',
          ]"
          @click="tab = 'translate'"
        >
            {{ t('settings.group.services') }}
          <span class="ml-1 text-[10px] text-muted-foreground">{{ props.state.services.length }}</span>
          <span
            v-if="tab === 'translate'"
            class="absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-primary"
          />
        </button>
        <button
          type="button"
          :class="[
            'relative flex items-center gap-1.5 px-2.5 pb-1.5 pt-0.5 text-xs font-medium transition-colors',
            tab === 'ocr'
              ? 'text-foreground'
              : 'text-muted-foreground hover:text-foreground',
          ]"
          @click="tab = 'ocr'"
        >
            {{ t('settings.group.ocr') }}
          <span class="text-[10px] text-muted-foreground">{{ props.state.ocrServices.length }}</span>
          <span
            v-if="tab === 'ocr'"
            class="absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-primary"
          />
        </button>
      </div>

      <!-- 翻译服务 Tab -->
      <template v-if="tab === 'translate'">
        <div class="flex shrink-0 items-center gap-2 rounded-md border border-input bg-background px-2.5">
          <Search class="h-3.5 w-3.5 text-muted-foreground" />
          <Input
            v-model="search"
            type="text"
              :placeholder="t('settings.placeholder.searchServices')"
            class="h-7 border-0 bg-transparent shadow-none focus-visible:ring-0 focus-visible:ring-offset-0 px-0"
          />
        </div>

        <div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-border bg-card">
          <ul class="min-h-0 flex-1 overflow-y-auto overscroll-contain divide-y divide-border scrollbar-thin">
            <li v-for="inst in filteredInstances" :key="inst.id">
              <div
                :class="[
                  'group relative flex items-center gap-2 px-2.5 py-2 transition-all duration-150 select-none',
                  'hover:bg-accent/40',
                  activeInstanceId === inst.id && 'bg-accent/60',
                  draggedId === inst.id && 'opacity-40',
                ]"
                :draggable="true"
                @dragstart="(e: DragEvent) => onDragStart(e, inst.id)"
                @dragover="(e: DragEvent) => onDragOver(e, inst.id)"
                @drop="(e: DragEvent) => onDrop(e, inst.id)"
                @dragend="onDragEnd"
              >
                <div
                  role="button"
                  tabindex="0"
                  class="flex flex-1 items-start gap-2.5 text-left min-w-0 self-center cursor-pointer"
                  @click="onServiceSelect(inst.id)"
                  @keydown.enter.prevent="onServiceSelect(inst.id)"
                  @keydown.space.prevent="onServiceSelect(inst.id)"
                >
                  <span
                    :class="[
                      'flex h-6 w-6 shrink-0 items-center justify-center rounded-md',
                      inst.enabled ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground',
                    ]"
                  >
                    <ServiceIcon :service-id="inst.type" class-name="h-3.5 w-3.5" />
                  </span>
                  <span class="flex-1 min-w-0">
                    <span class="block truncate text-[13px] font-medium text-foreground">
                      {{ inst.name }}
                    </span>
                    <span class="mt-0.5 block truncate text-[11px] leading-snug text-muted-foreground">
                      {{ inst.model || '—' }}
                    </span>
                  </span>
                </div>
                <div
                  class="flex shrink-0 self-center items-center gap-1.5"
                  @click.stop
                  @mousedown.stop
                >
                  <Badge
                    v-if="serviceById(inst.type)?.keyRequired === false"
                    variant="success"
                    class="h-4 px-1.5 text-[10px]"
                  :title="t('settings.tooltip.noApiKeyRequired')"
                  >
                  {{ t('settings.status.builtin') }}
                  </Badge>
                  <Badge
                    v-else
                    variant="warning"
                    class="h-4 px-1.5 text-[10px]"
                  :title="t('settings.tooltip.apiKeyRequired')"
                  >
                  {{ t('settings.status.keyRequired') }}
                  </Badge>
                  <span
                :title="isDeveloping(inst.type) ? t('settings.tooltip.developing') : undefined"
                    class="inline-flex"
                  >
                    <SettingSwitch
                      :model-value="inst.enabled"
                      :disabled="isDeveloping(inst.type)"
                  :aria-label="t(inst.enabled ? 'settings.aria.disableNamedService' : 'settings.aria.enableNamedService', { name: inst.name })"
                      @update:model-value="() => handleToggle(inst)"
                    />
                  </span>
                </div>
                <!-- Drop indicator(蓝色横线) -->
                <span
                  v-if="dropTargetId === inst.id && dropPosition === 'before'"
                  class="pointer-events-none absolute inset-x-0 -top-px h-0.5 rounded-full bg-primary"
                />
                <span
                  v-if="dropTargetId === inst.id && dropPosition === 'after'"
                  class="pointer-events-none absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-primary"
                />
              </div>
            </li>
            <li
              v-if="filteredInstances.length === 0"
              class="px-3 py-6 text-center text-xs text-muted-foreground"
            >
              {{ t('settings.empty.noMatchingServices') }}
            </li>
          </ul>
          <!-- 固定在左侧栏底部,不随列表滚动 -->
          <div class="shrink-0 border-t border-border p-2">
            <Dialog
              v-model:open="pickerOpen"
          :title="t('settings.button.addService')"
          :description="t('settings.description.addService')"
              width="640px"
            >
              <template #trigger>
                <Button variant="outline" size="sm" class="w-full">
                  <Plus class="h-3.5 w-3.5" />
              {{ t('settings.button.addService') }}
                </Button>
              </template>

              <div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
                <button
                  v-for="svc in addableServices"
                  :key="svc.id"
                  type="button"
                  class="group relative flex flex-col items-start gap-1.5 rounded-md border border-border bg-card p-2.5 text-left transition-colors hover:border-primary/40 hover:bg-accent/40"
                  @click="onAddService(svc.id)"
                >
                  <span class="flex h-7 w-7 items-center justify-center rounded-md bg-primary/10 text-primary group-hover:bg-primary/15">
                    <ServiceIcon :service-id="svc.id" class-name="h-3.5 w-3.5" />
                  </span>
                  <span class="text-xs font-medium text-foreground">{{ svc.name }}</span>
                  <span class="line-clamp-2 text-[10px] text-muted-foreground leading-snug">
                    {{ svc.description }}
                  </span>
                  <span
                    v-if="svc.protocols.length === 0"
                    class="absolute right-1.5 top-1.5 rounded bg-amber-100 px-1 py-0.5 text-[9px] font-normal text-amber-700 dark:bg-amber-900/40 dark:text-amber-300"
                    :title="t('settings.tooltip.developing')"
                  >
                    {{ t('common.developing') }}
                  </span>
                  <span
                    v-if="!svc.builtin"
                    class="absolute bottom-1.5 right-1.5 rounded bg-muted px-1 py-0.5 text-[9px] text-muted-foreground"
                  >
                    {{ t('settings.status.custom') }}
                  </span>
                </button>
              </div>
            </Dialog>
          </div>
        </div>
      </template>

      <!-- 文字识别 (OCR) Tab：system + 视觉实例列表；添加仅 vision picker -->
      <template v-else>
        <div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-border bg-card">
          <ul class="min-h-0 flex-1 overflow-y-auto overscroll-contain divide-y divide-border scrollbar-thin">
            <li v-for="inst in props.state.ocrServices" :key="inst.id">
              <div
                :class="[
                  'group relative flex items-center gap-2 px-2.5 py-2 transition-all duration-150',
                  'hover:bg-accent/40',
                  activeOcrInstanceId === inst.id && 'bg-accent/60',
                ]"
              >
                <div
                  role="button"
                  tabindex="0"
                  class="flex flex-1 items-start gap-2.5 text-left min-w-0 self-center cursor-pointer"
                  @click="onOcrSelect(inst.id)"
                  @keydown.enter.prevent="onOcrSelect(inst.id)"
                  @keydown.space.prevent="onOcrSelect(inst.id)"
                >
                  <span
                    :class="[
                      'flex h-6 w-6 shrink-0 items-center justify-center rounded-md',
                      inst.enabled ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground',
                    ]"
                  >
                    <ScanText class="h-3.5 w-3.5" />
                  </span>
                  <span class="flex-1 min-w-0">
                    <span class="block truncate text-[13px] font-medium text-foreground">
                      {{ inst.name }}
                    </span>
                    <span class="mt-0.5 block truncate text-[11px] leading-snug text-muted-foreground">
                      {{ ocrSubtitle(inst) }}
                    </span>
                  </span>
                </div>
                <div
                  class="flex shrink-0 self-center items-center gap-1.5"
                  @click.stop
                  @mousedown.stop
                >
                  <Badge
                    v-if="ocrServiceById(inst.type)?.keyRequired === false"
                    variant="success"
                    class="h-4 px-1.5 text-[10px]"
                    :title="t('settings.tooltip.noApiKeyRequired')"
                  >
                    {{ t('settings.status.builtin') }}
                  </Badge>
                  <Badge
                    v-else
                    variant="warning"
                    class="h-4 px-1.5 text-[10px]"
                    :title="t('settings.tooltip.apiKeyRequired')"
                  >
                    {{ t('settings.status.keyRequired') }}
                  </Badge>
                  <!-- canDisable=false（Windows 系统 OCR）：始终 on 且不可关 -->
                  <SettingSwitch
                    v-if="ocrServiceById(inst.type)?.canDisable === false"
                    :model-value="true"
                    disabled
                    :aria-label="t('settings.aria.enableNamedService', { name: inst.name })"
                    :title="t('settings.tooltip.ocrAlwaysEnabled')"
                  />
                  <SettingSwitch
                    v-else
                    :model-value="inst.enabled"
                    :aria-label="t(inst.enabled ? 'settings.aria.disableNamedService' : 'settings.aria.enableNamedService', { name: inst.name })"
                    @update:model-value="(v) => onOcrToggle(inst, v)"
                  />
                </div>
              </div>
            </li>
            <li
              v-if="props.state.ocrServices.length === 0"
              class="px-3 py-6 text-center text-xs text-muted-foreground"
            >
              {{ t('settings.empty.noMatchingServices') }}
            </li>
          </ul>
          <div class="shrink-0 border-t border-border p-2">
            <Dialog
              v-model:open="ocrPickerOpen"
              :title="t('settings.button.addOcrService')"
              :description="t(msgKey('settings.description.addOcrService'))"
              width="640px"
            >
              <template #trigger>
                <Button variant="outline" size="sm" class="w-full">
                  <Plus class="h-3.5 w-3.5" />
                  {{ t('settings.button.addOcrService') }}
                </Button>
              </template>

              <div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
                <button
                  v-for="svc in OCR_PICKER_SERVICES"
                  :key="svc.id"
                  type="button"
                  class="group relative flex flex-col items-start gap-1.5 rounded-md border border-border bg-card p-2.5 text-left transition-colors hover:border-primary/40 hover:bg-accent/40"
                  @click="onAddOcrService(svc.id)"
                >
                  <span class="flex h-7 w-7 items-center justify-center rounded-md bg-primary/10 text-primary group-hover:bg-primary/15">
                    <ScanText class="h-3.5 w-3.5" />
                  </span>
                  <span class="text-xs font-medium text-foreground">{{ svc.name }}</span>
                  <span class="line-clamp-2 text-[10px] text-muted-foreground leading-snug">
                    {{ svc.description }}
                  </span>
                </button>
              </div>
            </Dialog>
          </div>
        </div>
      </template>
    </aside>

    <!-- 右侧: OCR 详情（system / vision-llm） — 高度锁在网格行内,自身滚动 -->
    <div
      v-if="tab === 'ocr'"
      class="h-full min-h-0 min-w-0 self-stretch overflow-y-auto overscroll-contain pt-1 scrollbar-thin"
    >
      <div
        v-if="activeOcrInstance && activeOcrService"
        class="flex flex-col gap-2.5"
      >
        <!-- Header：system 不可重命名/删除；vision 可重命名 + 外链 + 删除走危险区 -->
        <header class="flex items-start gap-3">
          <span
            class="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary"
            :title="activeOcrService.detailKind === 'system' ? t('settings.tooltip.ocrAlwaysEnabled') : undefined"
          >
            <ScanText class="h-[18px] w-[18px]" />
          </span>
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <template v-if="!editingName || activeOcrService.detailKind === 'system'">
                <h2 class="truncate text-sm font-semibold text-foreground">
                  {{ activeOcrInstance.name }}
                </h2>
                <Button
                  v-if="activeOcrService.detailKind === 'vision-llm'"
                  variant="ghost"
                  size="icon"
                  class="h-6 w-6"
                  :aria-label="t('settings.aria.renameService')"
                  :title="t('settings.tooltip.rename')"
                  @click="enterNameEdit"
                >
                  <Pencil class="h-3 w-3" />
                </Button>
              </template>
              <template v-else>
                <input
                  ref="nameInput"
                  v-model="nameDraft"
                  type="text"
                  class="h-7 flex-1 max-w-[280px] rounded-md border border-input bg-background px-2 text-sm font-semibold text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                  @keydown.enter.prevent="commitNameEdit"
                  @keydown.esc.prevent="cancelNameEdit"
                  @blur="commitNameEdit"
                />
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-6 w-6"
                  :aria-label="t('common.confirm')"
                  @click="commitNameEdit"
                >
                  <Check class="h-3.5 w-3.5 text-primary" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-6 w-6"
                  :aria-label="t('common.cancel')"
                  @click="cancelNameEdit"
                >
                  <X class="h-3.5 w-3.5" />
                </Button>
              </template>
            </div>
            <p class="mt-0.5 text-[11px] leading-snug text-muted-foreground">
              {{ activeOcrService.description }}
            </p>
            <div
              v-if="
                activeOcrService.detailKind === 'vision-llm' &&
                (activeOcrService.docsUrl || activeOcrService.apiKeyUrl)
              "
              class="mt-2 flex flex-wrap items-center gap-1.5"
            >
              <button
                v-if="activeOcrService.docsUrl"
                type="button"
                class="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
                @click="openExternal(activeOcrService.docsUrl!)"
              >
                <BookOpen class="h-3 w-3" />
                {{ t(msgKey('settings.button.viewDocs')) }}
                <ExternalLink class="h-2.5 w-2.5 opacity-60" />
              </button>
              <button
                v-if="activeOcrService.apiKeyUrl"
                type="button"
                class="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
                @click="openExternal(activeOcrService.apiKeyUrl!)"
              >
                <KeyRound class="h-3 w-3" />
                {{ t(msgKey('settings.button.applyApiKey')) }}
                <ExternalLink class="h-2.5 w-2.5 opacity-60" />
              </button>
            </div>
          </div>
        </header>

        <!-- system：关于 + 三栏能力 + 状态条；醒目 configReserved -->
        <template v-if="activeOcrService.detailKind === 'system'">
          <div
            class="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2.5 text-xs text-amber-800 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-200"
          >
            <CircleAlert class="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>{{ t(msgKey('settings.ocr.configReserved')) }}</span>
          </div>

          <SettingGroup :title="t('settings.group.aboutService')">
            <SettingRow
              :title="t('settings.field.ocrEngine')"
              :description="t('settings.description.ocrImplementation')"
            >
              <code class="rounded bg-muted px-1.5 py-0.5 text-xs text-foreground/80">Windows.Media.Ocr</code>
            </SettingRow>
            <SettingRow
              :title="t('settings.field.networkRequirement')"
              :description="t('settings.description.networkRequirement')"
            >
              <span class="inline-flex items-center gap-1 text-xs text-foreground">
                <Lock class="h-3 w-3 text-muted-foreground" />
                {{ t('settings.status.offline') }}
              </span>
            </SettingRow>
            <SettingRow
              title="API Key"
              :description="t('settings.description.keyRequirement')"
            >
              <span class="text-xs text-foreground">{{ t('settings.status.noKeyRequired') }}</span>
            </SettingRow>
            <SettingRow
              :title="t('settings.field.canDisable')"
              :description="t('settings.description.canDisable')"
            >
              <span class="text-xs text-muted-foreground">{{ t('settings.status.systemService') }}</span>
            </SettingRow>
          </SettingGroup>

          <SettingGroup
            :title="t('settings.field.capabilities')"
            :description="t('settings.description.ocrCapabilities')"
          >
            <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
              <div class="rounded-md border border-border bg-background/40 p-3">
                <div class="flex items-center gap-1.5">
                  <Sparkles class="h-3 w-3 text-primary" />
                  <h4 class="text-xs font-medium text-foreground">{{ t('settings.ocr.commonCapabilities') }}</h4>
                </div>
                <p class="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">
                  {{ t('settings.ocr.languages') }}
                </p>
              </div>
              <div class="rounded-md border border-border bg-background/40 p-3">
                <div class="flex items-center gap-1.5">
                  <CircleAlert class="h-3 w-3 text-amber-600 dark:text-amber-400" />
                  <h4 class="text-xs font-medium text-foreground">{{ t('settings.ocr.limitationsTitle') }}</h4>
                </div>
                <p class="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">
                  {{ t('settings.ocr.limitations') }}
                </p>
              </div>
              <div class="rounded-md border border-border bg-background/40 p-3">
                <div class="flex items-center gap-1.5">
                  <ScanText class="h-3 w-3 text-primary" />
                  <h4 class="text-xs font-medium text-foreground">{{ t('settings.ocr.useCasesTitle') }}</h4>
                </div>
                <p class="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">
                  {{ t('settings.ocr.useCases') }}
                </p>
              </div>
            </div>
          </SettingGroup>

          <div
            class="flex items-center gap-2 rounded-md border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950/40 dark:text-emerald-200"
          >
            <ScanText class="h-3.5 w-3.5 shrink-0" />
            <span>{{ t('settings.ocr.footer') }}</span>
          </div>
        </template>

        <!-- vision-llm：缺 Key + 基础配置 + 高级提示词 + 删除 + 预留提示 -->
        <template v-else-if="activeOcrService.detailKind === 'vision-llm'">
          <div
            v-if="!activeOcrInstance.apiKey"
            class="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-200"
          >
            <CircleAlert class="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span class="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
              <span>{{ t('settings.warning.missingApiKey') }}</span>
              <button
                v-if="activeOcrService.apiKeyUrl"
                type="button"
                class="inline-flex items-center gap-1 underline-offset-2 hover:underline"
                @click="openExternal(activeOcrService.apiKeyUrl!)"
              >
                <KeyRound class="h-3 w-3" />
                {{ t(msgKey('settings.button.applyApiKey')) }}
                <ExternalLink class="h-2.5 w-2.5 opacity-60" />
              </button>
            </span>
          </div>

          <SettingGroup :title="t('settings.group.endpoint')" bare>
            <SettingRow
              title="API Endpoint"
              :description="t('settings.description.endpoint')"
              vertical
            >
              <SettingInput
                v-model="activeOcrInstance.endpoint"
                :placeholder="t('settings.placeholder.endpoint')"
              />
            </SettingRow>
          </SettingGroup>

          <SettingGroup :title="t('settings.group.credentials')" bare>
            <SettingRow
              title="API Key"
              :description="t('settings.description.apiKey', { name: activeOcrService.name })"
              vertical
            >
              <ApiKeyInput
                v-model="activeOcrInstance.apiKey"
                :status="activeOcrInstance.keyStatus"
                @validate="onOcrKeyValidate"
              />
            </SettingRow>
          </SettingGroup>

          <SettingGroup
            :title="t('settings.group.model')"
            :description="t('settings.description.model')"
            bare
          >
            <SettingRow
              :title="t('settings.field.defaultModel')"
              :description="t('settings.description.defaultModel')"
              vertical
            >
              <ModelCombobox
                :model-value="activeOcrInstance.model"
                :models="ocrModelOptions"
                :loading="pullingId === activeOcrInstance.id"
                :placeholder="t('settings.placeholder.model')"
                @update:model-value="(v) => (activeOcrInstance!.model = v)"
                @open="() => onOcrPullModels(activeOcrInstance!.id)"
              />
            </SettingRow>
          </SettingGroup>

          <div class="rounded-lg border border-border">
            <button
              type="button"
              class="flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left transition-colors hover:bg-accent/30"
              :aria-expanded="advancedOcrOpen"
              @click="advancedOcrOpen = !advancedOcrOpen"
            >
              <span class="flex min-w-0 items-center gap-2">
                <ChevronDown
                  :class="[
                    'h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform',
                    advancedOcrOpen ? 'rotate-0' : '-rotate-90',
                  ]"
                />
                <span class="text-xs font-medium text-foreground">
                  {{ t('settings.field.ocrPrompt') }}
                </span>
                <span class="truncate text-[11px] text-muted-foreground">
                  {{
                    isPromptDefault({
                      modelValue: activeOcrInstance.ocrPrompt,
                      defaultValue: DEFAULT_OCR_PROMPT,
                    })
                      ? t(msgKey('settings.prompt.summaryDefault'))
                      : t(msgKey('settings.prompt.summaryCustom'))
                  }}
                </span>
              </span>
            </button>
            <div v-if="advancedOcrOpen" class="space-y-3 border-t border-border px-3 py-3">
              <SettingTextarea
                :title="t('settings.field.ocrPrompt')"
                :description="t('settings.description.ocrPrompt')"
                :model-value="activeOcrInstance.ocrPrompt"
                :default-value="DEFAULT_OCR_PROMPT"
                @update:model-value="(v) => (activeOcrInstance!.ocrPrompt = v)"
              />
            </div>
          </div>

          <SettingGroup
            v-if="activeOcrService.canDelete !== false"
            :title="t('settings.group.danger')"
            bare
          >
            <SettingRow
              :title="t('settings.field.deleteService')"
              :description="t('settings.description.deleteService')"
            >
              <Button variant="outline" size="sm" @click="onOcrRemove">
                <Trash2 class="h-3.5 w-3.5 text-destructive" />
                {{ t('settings.button.deleteService') }}
              </Button>
            </SettingRow>
          </SettingGroup>

          <div
            class="flex items-start gap-2 rounded-md border border-border bg-muted/40 px-3 py-2.5 text-xs text-muted-foreground"
          >
            <CircleAlert class="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>{{ t(msgKey('settings.ocr.configReserved')) }}</span>
          </div>
        </template>
      </div>

      <!-- OCR 空态：无选中实例 -->
      <div
        v-else
        class="flex h-full min-h-[200px] items-center justify-center px-4 text-center text-xs text-muted-foreground"
      >
        {{ t('settings.empty.noMatchingServices') }}
      </div>
    </div>

    <div
      v-else-if="activeInstance && activeService"
      class="h-full min-h-0 min-w-0 self-stretch overflow-y-auto overscroll-contain pt-1 scrollbar-thin"
    >
      <div class="flex flex-col gap-2.5">
      <header class="flex items-start justify-between gap-3">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <template v-if="!editingName">
              <h2 class="truncate text-sm font-semibold text-foreground">
                {{ activeInstance.name }}
              </h2>
              <Button
                variant="ghost"
                size="icon"
                class="h-6 w-6"
                :aria-label="t('settings.aria.renameService')"
                :title="t('settings.tooltip.rename')"
                @click="enterNameEdit"
              >
                <Pencil class="h-3 w-3" />
              </Button>
              <span
                v-if="activeService.category === 'llm'"
                class="inline-flex items-center gap-0.5 text-[10px] font-normal text-muted-foreground/50 hover:text-muted-foreground transition-colors"
                :title="t('settings.tooltip.llmService')"
              >
                <Sparkles class="h-2.5 w-2.5" />
                AI
              </span>
            </template>
            <template v-else>
              <input
                ref="nameInput"
                v-model="nameDraft"
                type="text"
                class="h-7 flex-1 max-w-[280px] rounded-md border border-input bg-background px-2 text-sm font-semibold text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                @keydown.enter.prevent="commitNameEdit"
                @keydown.esc.prevent="cancelNameEdit"
                @blur="commitNameEdit"
              />
              <Button
                variant="ghost"
                size="icon"
                class="h-6 w-6"
                :aria-label="t('common.confirm')"
                @click="commitNameEdit"
              >
                <Check class="h-3.5 w-3.5 text-primary" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                class="h-6 w-6"
                :aria-label="t('common.cancel')"
                @click="cancelNameEdit"
              >
                <X class="h-3.5 w-3.5" />
              </Button>
            </template>
          </div>
          <p class="mt-0.5 text-[11px] leading-snug text-muted-foreground">
            {{ activeService.description }}
          </p>
          <div
            v-if="activeService.docsUrl || activeService.apiKeyUrl"
            class="mt-2 flex flex-wrap items-center gap-1.5"
          >
            <button
              v-if="activeService.docsUrl"
              type="button"
              class="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
              @click="openExternal(activeService.docsUrl!)"
            >
              <BookOpen class="h-3 w-3" />
              {{ t(msgKey('settings.button.viewDocs')) }}
              <ExternalLink class="h-2.5 w-2.5 opacity-60" />
            </button>
            <button
              v-if="activeService.apiKeyUrl"
              type="button"
              class="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
              @click="openExternal(activeService.apiKeyUrl!)"
            >
              <KeyRound class="h-3 w-3" />
              {{ t(msgKey('settings.button.applyApiKey')) }}
              <ExternalLink class="h-2.5 w-2.5 opacity-60" />
            </button>
          </div>
        </div>
      </header>

      <template v-if="activeService.id !== 'microsoft'">
      <div
        v-if="activeService.protocols.length === 0"
        class="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-200"
      >
        <CircleAlert class="mt-0.5 h-3.5 w-3.5 shrink-0" />
        <span>{{ t('settings.tooltip.developing') }}</span>
      </div>

      <SettingGroup :title="t('settings.group.endpoint')" bare>
        <SettingRow
          v-if="activeService?.protocols?.length"
          :title="t('settings.field.protocol')"
          :description="t('settings.description.protocol')"
          vertical
        >
          <template v-if="(activeService?.protocols?.length ?? 0) > 1">
            <select
              v-model="activeInstance.protocol"
              class="flex h-9 w-full rounded-md border border-input bg-background px-3 text-xs"
            >
              <option v-for="p in activeService?.protocols" :key="p.id" :value="p.id">
                {{ p.label }}
              </option>
            </select>
          </template>
          <span v-else class="text-xs text-foreground">
            {{ activeService?.protocols?.[0]?.label ?? '—' }}
          </span>
        </SettingRow>
        <SettingRow
          title="API Endpoint"
          :description="t('settings.description.endpoint')"
          vertical
        >
          <SettingInput
            v-model="activeInstance.endpoint"
            placeholder="https://api.openai.com/v1"
          />
        </SettingRow>
      </SettingGroup>

      <SettingGroup :title="t('settings.group.credentials')" bare>
        <SettingRow
          title="API Key"
          :description="t('settings.description.apiKey', { name: activeService.name })"
          vertical
        >
          <ApiKeyInput
              v-model="activeInstance.apiKey"
              :status="keyStatusFor(activeInstance.id)"
              @validate="onKeyValidate"
            />
        </SettingRow>
      </SettingGroup>

      <SettingGroup
        :title="t('settings.group.model')"
        :description="t('settings.description.model')"
        bare
      >
        <SettingRow
          :title="t('settings.field.defaultModel')"
          :description="t('settings.description.defaultModel')"
          vertical
        >
          <ModelCombobox
            :model-value="activeInstance.model"
            :models="modelOptions"
            :loading="pullingId === activeInstance.id"
            :placeholder="t('settings.placeholder.model')"
            @update:model-value="(v) => (activeInstance!.model = v)"
            @open="() => onPullModels(activeInstance!.id)"
          />
        </SettingRow>
      </SettingGroup>

      <!-- 高级：思维链 + 提示词 + custom 备注，默认折叠 -->
      <div
        v-if="activeService.category === 'llm' || activeService.id === 'custom'"
        class="rounded-lg border border-border"
      >
        <button
          type="button"
          class="flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left transition-colors hover:bg-accent/30"
          :aria-expanded="advancedOpen"
          @click="advancedOpen = !advancedOpen"
        >
          <span class="flex min-w-0 items-center gap-2">
            <ChevronDown
              :class="[
                'h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform',
                advancedOpen ? 'rotate-0' : '-rotate-90',
              ]"
            />
            <span class="text-xs font-medium text-foreground">{{ t(msgKey('settings.group.advancedPrompts')) }}</span>
            <span class="truncate text-[11px] text-muted-foreground">{{ advancedSummary }}</span>
          </span>
        </button>
        <div v-if="advancedOpen" class="space-y-3 border-t border-border px-3 py-3">
          <DevOnly>
            <SettingRow
              v-if="activeService.category === 'llm'"
              :title="t('settings.field.chainOfThought')"
              :description="t('settings.description.chainOfThought')"
              status="wip"
              vertical
            >
              <SettingSelect
                :model-value="activeInstance.chainOfThought"
                :options="chainOfThoughtOptions"
                @update:model-value="(v) => (activeInstance!.chainOfThought = v as typeof activeInstance.chainOfThought)"
              />
            </SettingRow>
          </DevOnly>

          <SettingTextarea
            v-if="activeService.category === 'llm'"
            :title="t('settings.field.systemPrompt')"
            :description="t('settings.description.systemPrompt')"
            :model-value="activeInstance.systemPrompt"
            :default-value="DEFAULT_PROMPTS.system"
            @update:model-value="(v) => (activeInstance!.systemPrompt = v)"
          />
          <SettingTextarea
            v-if="activeService.category === 'llm'"
            :title="t('settings.field.translationPrompt')"
            :description="t('settings.description.translationPrompt')"
            :variables="['{source_lang}', '{target_lang}', '{text}']"
            :model-value="activeInstance.translationPrompt"
            :default-value="DEFAULT_PROMPTS.translation"
            @update:model-value="(v) => (activeInstance!.translationPrompt = v)"
          />
          <DevOnly>
            <SettingTextarea
              v-if="activeService.category === 'llm'"
              :title="t('settings.field.reflectionPrompt')"
              :description="t('settings.description.reflectionPrompt')"
              status="wip"
              :model-value="activeInstance.reflectionPrompt"
              :default-value="DEFAULT_PROMPTS.reflection"
              :collapsed="!activeInstance.reflectionEnabled"
              :collapsed-hint="t(msgKey('settings.prompt.reflectionCollapsed'))"
              @update:model-value="(v) => (activeInstance!.reflectionPrompt = v)"
            >
              <template #header-end>
                <SettingSwitch
                  :model-value="activeInstance.reflectionEnabled"
                  @update:model-value="(v) => (activeInstance!.reflectionEnabled = v)"
                />
              </template>
            </SettingTextarea>
          </DevOnly>

          <SettingRow
            v-if="activeService.id === 'custom'"
            :title="t('settings.field.note')"
            :description="t('settings.description.note')"
            vertical
          >
            <SettingInput
              v-model="activeInstance.note"
              :placeholder="t('settings.placeholder.note')"
            />
          </SettingRow>
        </div>
      </div>
      </template>

      <SettingGroup :title="t('settings.group.danger')" bare>
        <SettingRow
          :title="t('settings.field.deleteService')"
          :description="t('settings.description.deleteService')"
        >
          <Button variant="outline" size="sm" @click="onRemove">
            <Trash2 class="h-3.5 w-3.5 text-destructive" />
            {{ t('settings.button.deleteService') }}
          </Button>
        </SettingRow>
      </SettingGroup>

      <div
        v-if="activeService.id !== 'microsoft' && !activeInstance.apiKey"
        class="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-200"
      >
        <CircleAlert class="h-3.5 w-3.5 mt-0.5 shrink-0" />
        <span class="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
          <span>{{ t('settings.warning.missingApiKey') }}</span>
          <button
            v-if="activeService.apiKeyUrl"
            type="button"
            class="inline-flex items-center gap-1 underline-offset-2 hover:underline"
            @click="openExternal(activeService.apiKeyUrl!)"
          >
            <KeyRound class="h-3 w-3" />
            {{ t(msgKey('settings.button.applyApiKey')) }}
            <ExternalLink class="h-2.5 w-2.5 opacity-60" />
          </button>
        </span>
      </div>
      </div>
    </div>
  </div>
</template>
