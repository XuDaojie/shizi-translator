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

} from '../components'
import type { AppSettings, ServiceId, ServiceInstance } from '../types'
import { DEFAULT_PROMPTS, MOCK_PULLED_MODELS, serviceById } from '../tokens'
import { useSettings } from '../stores/settings'

const props = defineProps<{
  state: AppSettings
}>()

const settings = useSettings()

/** 思维链长度档位,默认「关闭」对应 `chainOfThought: 'off'`。 */
const chainOfThoughtOptions = [
  { label: '关闭', value: 'off', description: '不生成推理过程,响应最快' },
  { label: '简短', value: 'short', description: '仅做关键步骤推理' },
  { label: '标准', value: 'medium', description: '常规推理深度(推荐)' },
  { label: '详细', value: 'long', description: '充分推理,适合复杂文本' },
]

const activeInstanceId = ref<string>(props.state.services[0]?.id ?? '')
const search = ref('')
const pickerOpen = ref(false)
const pulling = ref<Record<string, boolean>>({})
const tab = ref<'translate' | 'ocr'>('translate')

// 内置 + 用户自定义渠道的合并视图
const mergedServices = computed(() => settings.getMergedServices())

// 实例名称行内编辑
const editingName = ref(false)
const nameDraft = ref('')
const nameInput = ref<HTMLInputElement | null>(null)

const activeInstance = computed<ServiceInstance | undefined>(() =>
  props.state.services.find((s) => s.id === activeInstanceId.value),
)
const activeService = computed(() =>
  activeInstance.value ? serviceById(activeInstance.value.type) : undefined,
)

const filteredInstances = computed(() => {
  const q = search.value.trim().toLowerCase()
  const list = props.state.services
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
  () => activeInstance.value?.name,
  (n) => {
    if (!editingName.value) nameDraft.value = n ?? ''
  },
  { immediate: true },
)

const onServiceSelect = (id: string): void => {
  activeInstanceId.value = id
  editingName.value = false
}

const onKeyValidate = (key: string): void => {
  const inst = activeInstance.value
  if (!inst) return
  if (!key.trim()) {
    inst.keyStatus = 'invalid'
    toast.error('校验失败', '请先输入 API Key')
    return
  }
  if (inst.keyStatus === 'validating') return
  inst.keyStatus = 'validating'
  // mock:未对接真实接口;1.2s 后 50/50 展示 valid / invalid
  window.setTimeout(() => {
    if (activeInstance.value?.id !== inst.id) return
    if (Math.random() < 0.5) {
      inst.keyStatus = 'valid'
    } else {
      inst.keyStatus = 'invalid'
      toast.error('校验失败', 'API Key 无效或已过期,请检查后重试')
    }
  }, 1200)
}

const onRemove = (): void => {
  if (!activeInstance.value) return
  const removedId = activeInstance.value.id
  const name = activeInstance.value.name
  if (!window.confirm(`确认删除「${name}」？此操作不可撤销。`)) return
  settings.removeService(removedId)
  activeInstanceId.value = props.state.services[0]?.id ?? ''
}

const onAddService = (type: ServiceId): void => {
  const inst = settings.addService(type)
  activeInstanceId.value = inst.id
  pickerOpen.value = false
}

const onPullModels = async (instanceId: string): Promise<void> => {
  if (pulling.value[instanceId]) return
  const inst = props.state.services.find((s) => s.id === instanceId)
  if (!inst) return
  if (inst.pulledModels.length > 0) return
  pulling.value[instanceId] = true
  try {
    await new Promise<void>((r) => setTimeout(r, 1200))
    const incoming = MOCK_PULLED_MODELS[inst.type] ?? []
    inst.pulledModels = Array.from(new Set([...inst.pulledModels, ...incoming]))
  } finally {
    pulling.value[instanceId] = false
  }
}

const enterNameEdit = async (): Promise<void> => {
  if (!activeInstance.value) return
  nameDraft.value = activeInstance.value.name
  editingName.value = true
  await nextTick()
  nameInput.value?.focus()
  nameInput.value?.select()
}

const commitNameEdit = (): void => {
  if (!editingName.value || !activeInstance.value) return
  const next = nameDraft.value.trim()
  if (next && next !== activeInstance.value.name) {
    activeInstance.value.name = next
  }
  editingName.value = false
}

const cancelNameEdit = (): void => {
  editingName.value = false
  nameDraft.value = activeInstance.value?.name ?? ''
}

/** 拖拽重排:在 services 数组里拖动实例改变顺序。 */
const draggedId = ref<string | null>(null)
const dropTargetId = ref<string | null>(null)
const dropPosition = ref<'before' | 'after'>('before')

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
    <div class="grid grid-cols-[300px_1fr] gap-4 items-start">
    <!-- 左侧:服务实例列表(sticky,不随右侧编辑区滚动) -->
    <aside class="sticky top-4 flex max-h-[calc(100vh-2rem)] flex-col gap-3 overflow-y-auto scrollbar-thin">
      <!-- Tab 栏:翻译服务 / 文字识别 -->
      <div class="flex items-center gap-1 border-b border-border">
        <button
          type="button"
          :class="[
            'relative px-3 pb-2 pt-1 text-xs font-medium transition-colors',
            tab === 'translate'
              ? 'text-foreground'
              : 'text-muted-foreground hover:text-foreground',
          ]"
          @click="tab = 'translate'"
        >
          翻译服务
          <span class="ml-1 text-[10px] text-muted-foreground">{{ props.state.services.length }}</span>
          <span
            v-if="tab === 'translate'"
            class="absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-primary"
          />
        </button>
        <button
          type="button"
          :class="[
            'relative flex items-center gap-1.5 px-3 pb-2 pt-1 text-xs font-medium transition-colors',
            tab === 'ocr'
              ? 'text-foreground'
              : 'text-muted-foreground hover:text-foreground',
          ]"
          @click="tab = 'ocr'"
        >
          文字识别
          <span class="text-[10px] text-muted-foreground">1</span>
          <span
            class="rounded bg-amber-100 px-1 py-0.5 text-[9px] font-normal text-amber-700 dark:bg-amber-900/40 dark:text-amber-300"
            title="后续版本开放"
          >
            开发中
          </span>
          <span
            v-if="tab === 'ocr'"
            class="absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-primary"
          />
        </button>
      </div>

      <!-- 翻译服务 Tab -->
      <template v-if="tab === 'translate'">
        <div class="flex items-center gap-2 rounded-md border border-input bg-background px-2.5">
          <Search class="h-3.5 w-3.5 text-muted-foreground" />
          <Input
            v-model="search"
            type="text"
            placeholder="搜索服务"
            class="h-8 border-0 bg-transparent shadow-none focus-visible:ring-0 focus-visible:ring-offset-0 px-0"
          />
        </div>

        <div class="rounded-lg border border-border bg-card overflow-hidden">
          <ul class="max-h-[420px] overflow-y-auto divide-y divide-border scrollbar-thin">
            <li v-for="inst in filteredInstances" :key="inst.id">
              <div
                :class="[
                  'group relative flex items-center gap-2 px-3 py-2.5 transition-all duration-150',
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
                <button
                  type="button"
                  class="flex flex-1 items-start gap-3 text-left min-w-0 self-center"
                  @click="onServiceSelect(inst.id)"
                >
                  <span
                    :class="[
                      'flex h-7 w-7 shrink-0 items-center justify-center rounded-md',
                      inst.enabled ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground',
                    ]"
                  >
                    <ServiceIcon :service-id="inst.type" class-name="h-3.5 w-3.5" />
                  </span>
                  <span class="flex-1 min-w-0">
                    <span class="block text-sm font-medium text-foreground truncate">
                      {{ inst.name }}
                    </span>
                    <span class="mt-0.5 block text-[11px] leading-snug text-muted-foreground line-clamp-2">
                      {{ serviceById(inst.type)?.name ?? inst.type }} ·
                      {{ inst.apiKey ? '•••• 已配置' : '未配置' }}
                    </span>
                  </span>
                </button>
                <div
                  class="flex shrink-0 self-center items-center gap-1.5"
                  @click.stop
                  @mousedown.stop
                >
                  <Badge
                    v-if="serviceById(inst.type)?.keyRequired === false"
                    variant="success"
                    class="h-4 px-1.5 text-[10px]"
                    title="该服务无需 API Key,可直接使用"
                  >
                    内置
                  </Badge>
                  <Badge
                    v-else
                    variant="warning"
                    class="h-4 px-1.5 text-[10px]"
                    title="需要您填入 API Key 才能使用"
                  >
                    密钥
                  </Badge>
                  <SettingSwitch
                    :model-value="inst.enabled"
                    :aria-label="`${inst.enabled ? '停用' : '启用'} ${inst.name}`"
                    @update:model-value="(v) => (inst.enabled = v)"
                  />
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
              没有匹配的服务实例
            </li>
          </ul>
          <div class="border-t border-border p-2">
            <Dialog
              v-model:open="pickerOpen"
              title="添加服务"
              description="选择要添加的渠道,同渠道可创建多个实例(多 Key / 多账户)。"
              width="640px"
            >
              <template #trigger>
                <Button variant="outline" size="sm" class="w-full">
                  <Plus class="h-3.5 w-3.5" />
                  添加服务
                </Button>
              </template>

              <div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
                <button
                  v-for="svc in mergedServices"
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
                    v-if="!svc.builtin"
                    class="absolute right-1.5 top-1.5 rounded bg-muted px-1 py-0.5 text-[9px] text-muted-foreground"
                  >
                    自定义
                  </span>
                </button>
              </div>
            </Dialog>
          </div>
        </div>
      </template>

      <!-- 文字识别 (OCR) Tab -->
      <template v-else>
        <div class="rounded-lg border border-border bg-card overflow-hidden">
          <ul class="divide-y divide-border">
            <li>
              <div class="flex items-center gap-3 px-3 py-2.5">
                <span
                  class="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary"
                  title="Windows 媒体 OCR 始终启用,无法关闭"
                >
                  <ScanText class="h-3.5 w-3.5" />
                </span>
                <span class="flex-1 min-w-0">
                  <span class="flex items-center gap-1.5">
                    <span class="text-sm font-medium text-foreground truncate">
                      Windows 媒体 OCR
                    </span>
                    <span
                      class="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground"
                    >
                      内置
                    </span>
                  </span>
                  <span class="mt-0.5 block truncate text-[11px] text-muted-foreground">
                    Windows.Media.Ocr · 系统自带,始终可用
                  </span>
                </span>
              </div>
            </li>
          </ul>
          <div class="border-t border-border p-2">
            <div :title="'自定义 OCR 服务暂未开放,后续版本支持'">
              <Button
                variant="outline"
                size="sm"
                disabled
                class="w-full"
              >
                <Lock class="h-3.5 w-3.5" />
                添加 OCR 服务
              </Button>
            </div>
          </div>
        </div>
      </template>
    </aside>

    <!-- 右侧:实例详情(翻译服务) / OCR 信息面板(文字识别) -->
    <div v-if="tab === 'ocr'" class="flex flex-col gap-4 min-w-0 pt-14">
      <header class="flex items-start gap-3">
        <span
          class="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary"
          title="Windows 媒体 OCR 始终启用,无法关闭"
        >
          <ScanText class="h-[18px] w-[18px]" />
        </span>
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2 flex-wrap">
            <h2 class="text-base font-semibold text-foreground">Windows 媒体 OCR</h2>
            <span
              class="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground"
              title="随 Windows 10+ 系统预装,无需单独安装"
            >
              内置
            </span>
            <span
              class="inline-flex items-center gap-0.5 text-[10px] font-normal text-muted-foreground/50"
              title="大语言模型翻译,支持自定义推理深度"
            >
              <Sparkles class="h-2.5 w-2.5" />
              AI
            </span>
          </div>
          <p class="mt-1 text-xs text-muted-foreground leading-snug">
            Windows 10+ 系统自带的 OCR 引擎,通过 Windows.Media.Ocr API 提供。无需安装即可使用。
          </p>
        </div>
      </header>

      <SettingGroup title="关于此服务">
        <SettingRow
          title="OCR 引擎"
          description="当前 Windows 使用的光学字符识别实现"
        >
          <code class="rounded bg-muted px-1.5 py-0.5 text-xs text-foreground/80">Windows.Media.Ocr</code>
        </SettingRow>
        <SettingRow
          title="网络需求"
          description="是否需要联网才能完成识别"
        >
          <span class="inline-flex items-center gap-1 text-xs text-foreground">
            <Lock class="h-3 w-3 text-muted-foreground" />
            完全离线
          </span>
        </SettingRow>
        <SettingRow
          title="API Key"
          description="是否需要配置密钥才能使用"
        >
          <span class="text-xs text-foreground">无需密钥</span>
        </SettingRow>
        <SettingRow
          title="可关闭"
          description="是否允许在设置中停用该引擎"
        >
          <span class="text-xs text-muted-foreground">系统级服务,无法关闭</span>
        </SettingRow>
      </SettingGroup>

      <SettingGroup
        title="支持能力"
        description="按能力 / 局限 / 适合场景三栏对照,一目了然。无需 API Key,可直接使用。"
      >
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <div class="rounded-md border border-border bg-background/40 p-3">
            <div class="flex items-center gap-1.5">
              <Sparkles class="h-3 w-3 text-primary" />
              <h4 class="text-xs font-medium text-foreground">覆盖常见能力</h4>
            </div>
            <p class="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">
              印刷体中英日韩<br />
              简繁中文<br />
              西欧及多数东亚文字<br />
              <span class="text-foreground/80">共 60+ 语种</span>(可在 Windows 设置中下载更多)
            </p>
          </div>
          <div class="rounded-md border border-border bg-background/40 p-3">
            <div class="flex items-center gap-1.5">
              <CircleAlert class="h-3 w-3 text-amber-600 dark:text-amber-400" />
              <h4 class="text-xs font-medium text-foreground">已知局限</h4>
            </div>
            <p class="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">
              复杂版式识别率一般<br />
              手写体识别率较低<br />
              极端倾斜/低对比度可能丢字
            </p>
          </div>
          <div class="rounded-md border border-border bg-background/40 p-3">
            <div class="flex items-center gap-1.5">
              <ScanText class="h-3 w-3 text-primary" />
              <h4 class="text-xs font-medium text-foreground">适合场景</h4>
            </div>
            <p class="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">
              软件 UI 文字<br />
              菜单 / 按钮 / 对话框<br />
              网页正文 / 文档<br />
              简单排版的图片
            </p>
          </div>
        </div>
      </SettingGroup>

      <div
        class="flex items-center gap-2 rounded-md border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950/40 dark:text-emerald-200"
      >
        <ScanText class="h-3.5 w-3.5 shrink-0" />
        <span>由 Windows 系统提供 · 始终可用 · 无需网络 · 截图后即时识别</span>
      </div>
    </div>

    <div v-else-if="activeInstance && activeService" class="flex flex-col gap-4 min-w-0 pt-14">
      <header class="flex items-start justify-between gap-4">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <template v-if="!editingName">
              <h2 class="text-base font-semibold text-foreground truncate">
                {{ activeInstance.name }}
              </h2>
              <Button
                variant="ghost"
                size="icon"
                class="h-6 w-6"
                aria-label="重命名实例"
                title="重命名"
                @click="enterNameEdit"
              >
                <Pencil class="h-3 w-3" />
              </Button>
              <span
                v-if="activeService.category === 'llm'"
                class="inline-flex items-center gap-0.5 text-[10px] font-normal text-muted-foreground/50 hover:text-muted-foreground transition-colors"
                title="大语言模型翻译,支持自定义推理深度"
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
                aria-label="确认"
                @click="commitNameEdit"
              >
                <Check class="h-3.5 w-3.5 text-primary" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                class="h-6 w-6"
                aria-label="取消"
                @click="cancelNameEdit"
              >
                <X class="h-3.5 w-3.5" />
              </Button>
            </template>
          </div>
          <p class="mt-1 text-xs text-muted-foreground leading-snug">
            {{ activeService.description }}
          </p>
        </div>
      </header>

      <SettingGroup v-if="activeService.needsEndpoint" title="接入点" bare>
        <SettingRow
          title="API Endpoint"
          description="OpenAI 兼容协议的完整地址,例如 https://api.openai.com/v1。"
          vertical
        >
          <SettingInput
            v-model="activeInstance.endpoint"
            placeholder="https://api.openai.com/v1"
          />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="凭据" bare>
        <SettingRow
          title="API Key"
          :description="`用于调用 ${activeService.name} 接口,本地加密存储,不会上传。`"
          vertical
        >
          <ApiKeyInput
              v-model="activeInstance.apiKey"
              :status="activeInstance.keyStatus"
              @validate="onKeyValidate"
            />
        </SettingRow>
      </SettingGroup>

      <SettingGroup
        title="模型"
        description="打开下拉自动从服务商拉取模型列表;输入即搜索,可直接键入列表外的自定义模型名(如自动路由、私有灰度模型)。"
        bare
      >
        <SettingRow
          title="默认模型"
          description="可在调用时临时覆盖。"
          vertical
        >
          <ModelCombobox
            :model-value="activeInstance.model"
            :models="activeInstance.pulledModels"
            :loading="pulling[activeInstance.id] ?? false"
            :placeholder="activeService.hasModelApi ? '打开下拉拉取模型,或直接输入' : '请输入模型名'"
            @update:model-value="(v) => (activeInstance!.model = v)"
            @open="() => onPullModels(activeInstance!.id)"
          />
        </SettingRow>
      </SettingGroup>

      <SettingGroup
        v-if="activeService.category === 'llm'"
        title="思维链"
        bare
      >
        <SettingRow
          title="思维链长度"
          description="控制大模型回答前的推理深度。关闭后模型直接输出翻译结果,不会生成推理过程。"
          status="wip"
          vertical
        >
          <SettingSelect
            :model-value="activeInstance.chainOfThought"
            :options="chainOfThoughtOptions"
            @update:model-value="(v) => (activeInstance!.chainOfThought = v as typeof activeInstance.chainOfThought)"
          />
        </SettingRow>
      </SettingGroup>

      <SettingGroup
        v-if="activeService.category === 'llm'"
        title="提示词"
        bare
      >
        <SettingRow
          title="系统提示词"
          description="设定 AI 翻译时的角色、语气与术语规则。每次对话都会作为 system role 发送给模型。"
          status="wip"
          vertical
        >
          <SettingTextarea
            :model-value="activeInstance.systemPrompt"
            :default-value="DEFAULT_PROMPTS.system"
            placeholder="系统提示词"
            @update:model-value="(v) => (activeInstance!.systemPrompt = v)"
          />
        </SettingRow>
        <SettingRow
          title="翻译提示词"
          description="实际翻译时使用的模板,支持占位符 {source_lang}(源语言)、{target_lang}(目标语言)、{text}(待翻译文本)。"
          status="wip"
          vertical
        >
          <SettingTextarea
            :model-value="activeInstance.translationPrompt"
            :default-value="DEFAULT_PROMPTS.translation"
            placeholder="翻译提示词"
            @update:model-value="(v) => (activeInstance!.translationPrompt = v)"
          />
        </SettingRow>
        <SettingRow
          title="反思提示词"
          description="译后让模型对译文做一次自检并改进,提升准确度与一致性。会增加一次模型调用与耗时。"
          status="wip"
          vertical
        >
          <div class="flex w-full items-center justify-between gap-3">
            <p class="text-xs text-muted-foreground">
              {{ activeInstance.reflectionEnabled ? '已启用,译后会执行一次自检' : '默认关闭,启用后译文质量更稳但响应更慢' }}
            </p>
            <SettingSwitch
              :model-value="activeInstance.reflectionEnabled"
              :title="activeInstance.reflectionEnabled ? '关闭反思' : '启用反思'"
              @update:model-value="(v) => (activeInstance!.reflectionEnabled = v)"
            />
          </div>
          <SettingTextarea
            v-if="activeInstance.reflectionEnabled"
            :model-value="activeInstance.reflectionPrompt"
            :default-value="DEFAULT_PROMPTS.reflection"
            placeholder="反思提示词"
            class="mt-3"
            @update:model-value="(v) => (activeInstance!.reflectionPrompt = v)"
          />
        </SettingRow>
      </SettingGroup>

      <SettingGroup v-if="activeService.id === 'custom'" title="备注" bare>
        <SettingRow
          title="备注"
          description="为这个自定义实例起一个易记的名字,方便区分多个端点。"
          vertical
        >
          <SettingInput
            v-model="activeInstance.note"
            placeholder="例如:本地 Ollama / 公司内部代理"
          />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="危险操作" bare>
        <SettingRow
          title="删除此服务实例"
          description="将同时停用实例、清空其 API Key / Endpoint / 模型选择,不可恢复。"
        >
          <Button variant="outline" size="sm" @click="onRemove">
            <Trash2 class="h-3.5 w-3.5 text-destructive" />
            删除实例
          </Button>
        </SettingRow>
      </SettingGroup>

      <div
        v-if="!activeInstance.apiKey"
        class="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-200"
      >
        <CircleAlert class="h-3.5 w-3.5 mt-0.5 shrink-0" />
        <span>未配置 API Key,该实例将无法用于翻译。可前往对应服务商申请。</span>
      </div>
    </div>
  </div>
</template>
