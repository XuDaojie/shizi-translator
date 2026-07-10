import { reactive, watch } from 'vue'
import type {
  AppSettings,
  CustomServiceType,
  LogLevel,
  OcrHistoryEntry,
  ServiceId,
  ServiceInstance,
  ServiceMeta,
} from '../types'
import type { AppConfig, ServiceInstanceConfig } from '@/types/config'
import { BUILTIN_SERVICES, buildServices, DEFAULT_PROMPTS } from '../tokens'
import { projectToAppConfig, validateConfig } from '@/lib/config'
import { invokeGetAppConfig, invokeGetShortcutConflicts, invokeSaveAppConfig, isTauriReady, type ShortcutConflict } from '@/lib/tauri'
import { toast } from '@/lib/toast'
import { createLogger } from '@public/logger.js'

const STORAGE_KEY = 'app:settings:v1'
const logger = createLogger('settings')
/** 旧版本 key,首次启动时如有残留则迁移到新 key,确保用户数据不丢。 */
const LEGACY_STORAGE_KEYS = ['shizi:settings:v1']

/** 浏览器原生 ID 生成,无外部依赖,失败时回退到基于时间戳 + 计数器的伪 id。 */
const newInstanceId = (): string => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID()
  }
  return `inst-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

/** 历史记录 id 同理,前缀 `hist-` 方便排查。 */
const newHistoryId = (): string => `hist-${newInstanceId().slice(5)}`

const firstAvailableProtocol = (meta?: ServiceMeta) =>
  meta?.protocols.find((p) => p.status === 'available')

const defaultInstanceFor = (type: ServiceId, name: string, enabled = false): ServiceInstance => {
  const meta = BUILTIN_SERVICES.find((s) => s.id === type)
  const protocol = firstAvailableProtocol(meta)
  return {
    id: newInstanceId(),
    type,
    name,
    enabled,
    // protocols 为空的渠道（deepl 等 ML）走 'openai_chat' 占位；
    // 这类渠道在 ServicesPanel 启用开关被置灰，不会进入翻译批次，protocol 值不影响运行。
    protocol: protocol?.id ?? 'openai_chat',
    apiKey: '',
    model: protocol?.defaultModel ?? meta?.defaultModel ?? '',
    endpoint: protocol?.defaultEndpoint ?? '',
    note: '',
    pulledModels: [],
    keyStatus: 'idle',
    chainOfThought: 'off',
    systemPrompt: DEFAULT_PROMPTS.system,
    translationPrompt: DEFAULT_PROMPTS.translation,
    reflectionPrompt: DEFAULT_PROMPTS.reflection,
    reflectionEnabled: false,
  }
}

/** 首启仅展示 DeepSeek 和智谱 AI，默认关闭。其余渠道由用户自行添加。 */
const seedInstances = (): ServiceInstance[] =>
  ['deepseek', 'zhipu']
    .map((id) => BUILTIN_SERVICES.find((s) => s.id === id))
    .filter((m): m is ServiceMeta => !!m)
    .map((svc) => defaultInstanceFor(svc.id, svc.name, false))

/**
 * 默认 OCR 历史样本,首次启动(无 localStorage)时展示。
 * 覆盖"今天/昨天/本周/更早"四个时间桶,每条都是真实场景里能识别的常见英文/日文/韩文 UI 文案。
 * 时间戳基于当前时刻偏移,刷新后时间仍合理。
 */
const seedOcrHistory = (): OcrHistoryEntry[] => {
  return []
}

const buildDefaults = (): AppSettings => {
  const instances = seedInstances()
  return {
    general: {
      launchAtLogin: true,
      startMinimized: false,
      showTrayIcon: true,
      closeAction: 'minimize',
      popupPrecreate: true,
      overlayPrecreate: true,
      theme: 'system',
      language: 'zh-CN',
      updateChannel: 'stable',
      autoCheckUpdate: true,
    },
    translation: {
      defaultSourceLang: 'auto',
      defaultTargetLang: 'zh-CN',
      autoCopy: true,
      restoreClipboard: true,
      autoPaste: false,
      showPhonetic: true,
      showAlternatives: true,
      autoDetect: true,
      wordLookupDelay: 300,
      historyLimit: 500,
    },
    shortcut: {
      bindings: [
        {
          id: 'translate-selection',
          label: '划词翻译',
          description: '选中任意文本后按下快捷键即可翻译。',
          keys: 'Alt+D',
        },
        {
          id: 'translate-clipboard',
          label: '剪贴板翻译',
          description: '直接翻译当前剪贴板中的内容。',
          keys: 'Ctrl+Shift+C',
        },
        {
          id: 'translate-screenshot',
          label: '截图翻译',
          description: '框选屏幕区域后识别并翻译其中的文字。',
          keys: 'Alt+E',
        },
        {
          id: 'word-lookup',
          label: '取词翻译',
          description: '光标悬停在词语上时弹出翻译结果。',
          keys: '',
        },
        {
          id: 'open-settings',
          label: '打开设置',
          description: '直接打开设置页面。',
          keys: 'Ctrl+,',
        },
      ],
    },
    services: instances,
    customServiceTypes: [],
    advanced: {
      logLevel: 'info',
      betaLookup: false,
      betaVoice: false,
      collectUsage: true,
    },
    ocrHistory: seedOcrHistory(),
  }
}

const isLegacyRecord = (raw: unknown): raw is Record<ServiceId, ServiceInstance> =>
  !!raw && typeof raw === 'object' && !Array.isArray(raw) && 'openai' in (raw as object)

/**
 * 将旧版本 `Record<ServiceId, ServiceConfig>` 数据迁移为 `ServiceInstance[]`。
 * 旧结构里每个渠道只有一条,迁移时为每条生成一个 instance,name 取渠道 meta name。
 */
const migrateLegacyServices = (
  legacy: Record<ServiceId, ServiceInstance>,
): ServiceInstance[] => {
  const result: ServiceInstance[] = []
  for (const svc of BUILTIN_SERVICES) {
    const old = legacy[svc.id] as Partial<ServiceInstance> | undefined
    result.push({
      id: newInstanceId(),
      type: svc.id,
      name: svc.name,
      enabled: old?.enabled ?? false,
      protocol: firstAvailableProtocol(svc)?.id ?? 'openai_chat',
      apiKey: old?.apiKey ?? '',
      model: old?.model ?? svc.defaultModel ?? '',
      endpoint: old?.endpoint ?? '',
      note: old?.note ?? '',
      pulledModels: old?.pulledModels ?? [],
      keyStatus: old?.keyStatus ?? 'idle',
      chainOfThought: old?.chainOfThought ?? 'off',
      systemPrompt: old?.systemPrompt ?? DEFAULT_PROMPTS.system,
      translationPrompt: old?.translationPrompt ?? DEFAULT_PROMPTS.translation,
      reflectionPrompt: old?.reflectionPrompt ?? DEFAULT_PROMPTS.reflection,
      reflectionEnabled: old?.reflectionEnabled ?? false,
    })
  }
  return result
}

/** slug 化 name:小写 + 仅 [a-z0-9_-],首字符必须是 a-z,2-30 字符。 */
const slugify = (name: string): string =>
  name
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .replace(/-{2,}/g, '-')
    .slice(0, 30)

/** 新建用户自定义渠道 id:`custom_<slug>_<4位随机>`,与内置 id 区分(内置都是单词)。 */
const newCustomTypeId = (name: string): string => {
  const slug = slugify(name) || 'type'
  const tail = Math.random().toString(36).slice(2, 6)
  return `custom_${slug}_${tail}`
}

/** 后端 ServiceInstanceConfig -> 前端 ServiceInstance,前端独有字段用默认值。 */
const backendInstanceToLocal = (backend: ServiceInstanceConfig): ServiceInstance => ({
  id: backend.id,
  type: backend.serviceType as ServiceId,
  name: backend.name,
  enabled: backend.enabled,
  protocol: backend.protocol,
  apiKey: backend.apiKey ?? '',
  model: backend.model,
  endpoint: backend.endpoint,
  note: '',
  pulledModels: [],
  keyStatus: 'idle',
  chainOfThought: backend.chainOfThought || 'off',
  systemPrompt: backend.systemPrompt ?? DEFAULT_PROMPTS.system,
  translationPrompt: backend.translationPrompt ?? DEFAULT_PROMPTS.translation,
  reflectionPrompt: backend.reflectionPrompt ?? DEFAULT_PROMPTS.reflection,
  reflectionEnabled: backend.reflectionEnabled,
})

export const mergeBackendIntoServices = (
  local: ServiceInstance[],
  backend: ServiceInstanceConfig[],
): ServiceInstance[] => {
  const localById = new Map(local.map((s) => [s.id, s]))

  return backend.map((b) => {
    const existing = localById.get(b.id)
    if (existing) {
      return {
        ...existing,
        enabled: b.enabled,
        apiKey: b.apiKey ?? '',
        endpoint: b.endpoint,
        model: b.model,
        protocol: b.protocol,
        systemPrompt: b.systemPrompt ?? existing.systemPrompt,
        translationPrompt: b.translationPrompt ?? existing.translationPrompt,
        reflectionPrompt: b.reflectionPrompt ?? existing.reflectionPrompt,
        reflectionEnabled: b.reflectionEnabled,
        chainOfThought: b.chainOfThought,
      }
    }
    return backendInstanceToLocal(b)
  })
}


const mergeBackendIntoShortcuts = (
  local: AppSettings['shortcut']['bindings'],
  backend: AppConfig['shortcuts'],
): AppSettings['shortcut']['bindings'] =>
  local.map((binding) => ({
    ...binding,
    keys: Object.prototype.hasOwnProperty.call(backend, binding.id)
      ? backend[binding.id]
      : binding.keys,
    error: undefined,
  }))

/** 把后端快捷键冲突按 id 写入对应 binding.error；未列出的清空。 */
export const applyShortcutConflicts = (
  bindings: AppSettings['shortcut']['bindings'],
  conflicts: ShortcutConflict[],
): AppSettings['shortcut']['bindings'] => {
  const byId = new Map(conflicts.map((c) => [c.id, c.message]))
  return bindings.map((binding) => ({
    ...binding,
    error: byId.has(binding.id) ? byId.get(binding.id) : undefined,
  }))
}

const VALID_LOG_LEVELS: readonly LogLevel[] = ['error', 'warn', 'info', 'debug']

/** 后端 logLevel 有效则覆盖前端，否则保留前端。 */
export const applyBackendLogLevel = (
  local: LogLevel,
  backend: string | undefined,
): LogLevel =>
  backend && VALID_LOG_LEVELS.includes(backend as LogLevel)
    ? (backend as LogLevel)
    : local

const loadFromStorage = (): AppSettings => {
  if (typeof window === 'undefined') return buildDefaults()
  try {
    let raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) {
      // 旧 key 自动迁移:读取并删除旧条目,后续写入新 key
      for (const oldKey of LEGACY_STORAGE_KEYS) {
        const oldRaw = window.localStorage.getItem(oldKey)
        if (oldRaw) {
          window.localStorage.setItem(STORAGE_KEY, oldRaw)
          window.localStorage.removeItem(oldKey)
          raw = oldRaw
          break
        }
      }
    }
    if (!raw) return buildDefaults()
    const parsed = JSON.parse(raw) as Partial<AppSettings>
    const defaults = buildDefaults()

    let services: ServiceInstance[]
    if (Array.isArray(parsed.services)) {
      // 新结构:用持久化的;若空数组(用户清空)也保持空。
      const incoming = parsed.services.length > 0 ? parsed.services : defaults.services
      // 旧版本 localStorage 里可能没有 keyStatus / chainOfThought,补默认值
      services = incoming.map((s) => ({
        ...s,
        keyStatus: s.keyStatus ?? 'idle',
        chainOfThought: s.chainOfThought ?? 'off',
        systemPrompt: s.systemPrompt ?? DEFAULT_PROMPTS.system,
        translationPrompt: s.translationPrompt ?? DEFAULT_PROMPTS.translation,
        reflectionPrompt: s.reflectionPrompt ?? DEFAULT_PROMPTS.reflection,
        reflectionEnabled: s.reflectionEnabled ?? false,
      }))
    } else if (isLegacyRecord(parsed.services)) {
      // 旧结构:迁移
      services = migrateLegacyServices(parsed.services)
    } else {
      services = defaults.services
    }

    return {
      general: { ...defaults.general, ...parsed.general },
      translation: {
        ...defaults.translation,
        ...parsed.translation,
      },
      shortcut: {
        // 以 defaults 为事实来源，丢弃已移除的绑定（如旧版 show-window）
        bindings: defaults.shortcut.bindings.map((def) => {
          const saved = parsed.shortcut?.bindings?.find((b) => b.id === def.id)
          return saved ? { ...def, ...saved, id: def.id, label: def.label, description: def.description } : def
        }),
      },
      services,
      customServiceTypes: parsed.customServiceTypes ?? [],
      advanced: { ...defaults.advanced, ...parsed.advanced },
      // 旧版本无 ocrHistory 字段,backfill seed 展示;若已存在(空数组也保留)用本地值
      ocrHistory: parsed.ocrHistory ?? defaults.ocrHistory,
    }
  } catch {
    return buildDefaults()
  }
}

const state = reactive<AppSettings>(loadFromStorage())

const dirty = reactive({ value: false })
const saveStatus = reactive<{ value: 'idle' | 'saved' | 'saving' | 'error' }>({ value: 'idle' })
const baseline = JSON.parse(JSON.stringify(state)) as AppSettings
let autoSaveTimer: ReturnType<typeof setTimeout> | undefined
let saveStatusIdleTimer: ReturnType<typeof setTimeout> | undefined
let syncingFromBackend = false

/**
 * 从后端拉取快捷键冲突并写入对应 binding.error。失败静默——冲突信息非关键。
 * 调用方需自行用 syncingFromBackend 包裹，避免冲突展示触发自动保存。
 */
const refreshShortcutConflicts = async (): Promise<void> => {
  try {
    const conflicts = await invokeGetShortcutConflicts()
    state.shortcut.bindings = applyShortcutConflicts(state.shortcut.bindings, conflicts ?? [])
  } catch {
    // 忽略：冲突信息非关键
  }
}

/**
 * 把状态序列化为 stable 字符串,排除 ocrHistory(它是"持久化数据"而非"待保存设置",
 * 不应触发 footer 的"放弃/保存"按钮)。
 */
const serializeForDirty = (s: AppSettings): string =>
  JSON.stringify({
    ...s,
    ocrHistory: undefined,
    services: s.services.map((service) => ({ ...service, keyStatus: 'idle' })),
    // error 是运行时冲突展示状态，不应触发"未保存"标记
    shortcut: {
      bindings: s.shortcut.bindings.map((b) => ({ ...b, error: undefined })),
    },
  })

const markDirty = (): void => {
  dirty.value = serializeForDirty(state) !== serializeForDirty(baseline)
}

watch(state, markDirty, { deep: true })

const cloneSettings = (s: AppSettings): AppSettings => JSON.parse(JSON.stringify(s)) as AppSettings

const commitBaseline = (next: AppSettings = state): void => {
  Object.assign(baseline, cloneSettings(next))
  dirty.value = false
}

const showSavedBriefly = (): void => {
  saveStatus.value = 'saved'
  if (saveStatusIdleTimer) clearTimeout(saveStatusIdleTimer)
  saveStatusIdleTimer = setTimeout(() => {
    if (saveStatus.value === 'saved') saveStatus.value = 'idle'
  }, 2400)
}

const persist = async (notify = false): Promise<void> => {
  const snapshot = cloneSettings(state)
  const config = projectToAppConfig(snapshot)
  const err = validateConfig(config)
  if (err) {
    saveStatus.value = 'error'
    toast.error('保存失败', err)
    logger.warn('配置校验失败', err)
    return
  }
  try {
    if (isTauriReady()) {
      await invokeSaveAppConfig(config)
      if (notify) toast.success('配置已保存')
    } else if (notify) {
      toast.info('Tauri 未就绪，仅本地保存')
    }
    if (serializeForDirty(state) === serializeForDirty(snapshot)) {
      commitBaseline(snapshot)
      showSavedBriefly()
    } else {
      markDirty()
      saveStatus.value = 'saving'
    }
  } catch (e) {
    saveStatus.value = 'error'
    toast.error('保存失败', String(e))
    logger.error('保存配置失败', String(e))
  }
}

watch(
  state,
  (next) => {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
    if (syncingFromBackend) return
    markDirty()
    if (!dirty.value) return
    saveStatus.value = 'saving'
    if (autoSaveTimer) clearTimeout(autoSaveTimer)
    if (saveStatusIdleTimer) clearTimeout(saveStatusIdleTimer)
    autoSaveTimer = setTimeout(() => void persist(), 300)
  },
  { deep: true, flush: 'sync' },
)

const applyTheme = (): void => {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  const prefersDark =
    typeof window !== 'undefined' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches
  const resolved =
    state.general.theme === 'system' ? (prefersDark ? 'dark' : 'light') : state.general.theme
  root.classList.toggle('dark', resolved === 'dark')
}

watch(() => state.general.theme, applyTheme, { immediate: true })
watch(() => state.advanced.logLevel, (v) => logger.setLevel(v))

/** 在指定 type 下生成下一个默认 name:`OpenAI`、`OpenAI 2`、`OpenAI 3` ... */
const nextDefaultName = (type: ServiceId): string => {
  const meta = buildServices(state.customServiceTypes).find((s) => s.id === type)
  const base = meta?.name ?? type
  const sameType = state.services.filter((s) => s.type === type).length
  return sameType === 0 ? base : `${base} ${sameType + 1}`
}

export const useSettings = () => ({
  state,
  dirty,
  saveStatus,
  async save(): Promise<void> {
    if (autoSaveTimer) clearTimeout(autoSaveTimer)
    saveStatus.value = 'saving'
    await persist(true)
  },
  /** 启动时从后端 config.json 同步：后端空则推前端覆盖，后端非空则按 id 合并。失败静默降级。 */
  async syncFromBackend(): Promise<void> {
    if (!isTauriReady()) return
    let backend: AppConfig
    try {
      backend = await invokeGetAppConfig()
    } catch {
      logger.warn('从后端同步配置失败')
      return
    }
    if (!backend.services || backend.services.length === 0) {
      // 后端空（旧格式残留 / 首次启动）→ 前端推后端覆盖
      try {
        await invokeSaveAppConfig(projectToAppConfig(state))
      } catch {
        logger.warn('推送配置到后端失败')
        // 忽略：下次启动再试
      }
      syncingFromBackend = true
      await refreshShortcutConflicts()
      syncingFromBackend = false
      return
    }
    syncingFromBackend = true
    state.services = mergeBackendIntoServices(state.services, backend.services)
    state.translation.defaultSourceLang =
      backend.defaultSourceLang ?? state.translation.defaultSourceLang
    state.translation.defaultTargetLang =
      backend.targetLang ?? state.translation.defaultTargetLang
    state.translation.autoCopy = backend.autoCopy ?? state.translation.autoCopy
    state.translation.restoreClipboard =
      backend.restoreClipboard ?? state.translation.restoreClipboard
    state.translation.historyLimit =
      backend.historyLimit ?? state.translation.historyLimit
    state.shortcut.bindings = mergeBackendIntoShortcuts(state.shortcut.bindings, backend.shortcuts ?? {})
    state.advanced.logLevel = applyBackendLogLevel(
      state.advanced.logLevel,
      backend.logLevel,
    )
    logger.setLevel(state.advanced.logLevel)
    await refreshShortcutConflicts()
    syncingFromBackend = false
    commitBaseline()
    saveStatus.value = 'idle'
  },
  reset(): void {
    const defaults = buildDefaults()
    Object.assign(state, defaults)
  },
  discard(): void {
    Object.assign(state, JSON.parse(JSON.stringify(baseline)))
  },
  /** 在 services 数组末尾添加一条新实例并返回它。 */
  addService(type: ServiceId): ServiceInstance {
    const inst = defaultInstanceFor(type, nextDefaultName(type))
    state.services.push(inst)
    return inst
  },
  removeService(instanceId: string): void {
    const idx = state.services.findIndex((s) => s.id === instanceId)
    if (idx < 0) return
    state.services.splice(idx, 1)
  },
  renameService(instanceId: string, name: string): void {
    const inst = state.services.find((s) => s.id === instanceId)
    if (!inst) return
    inst.name = name.trim() || inst.name
  },
  /**
   * 拖拽重排实例顺序。`position: 'after'` 时插入到 `toId` 之后,`'before'` 插入到其之前。
   * 同 id / 越界静默 no-op,不对输入做校验(由调用方负责)。
   */
  reorderService(fromId: string, toId: string, position: 'before' | 'after' = 'before'): void {
    const fromIdx = state.services.findIndex((s) => s.id === fromId)
    if (fromIdx < 0) return
    const [moved] = state.services.splice(fromIdx, 1)
    const toIdx = state.services.findIndex((s) => s.id === toId)
    if (toIdx < 0) {
      state.services.push(moved)
      return
    }
    const insertAt = position === 'after' ? toIdx + 1 : toIdx
    state.services.splice(insertAt, 0, moved)
  },
  findInstance(instanceId: string): ServiceInstance | undefined {
    return state.services.find((s) => s.id === instanceId)
  },
  /** 注册一个用户自定义渠道类型;返回新 id,重复则抛错。 */
  addCustomServiceType(name: string): CustomServiceType {
    const clean = name.trim()
    if (!clean) throw new Error('渠道名不能为空')
    const id = newCustomTypeId(clean)
    const all = buildServices(state.customServiceTypes)
    if (all.some((s) => s.name.toLowerCase() === clean.toLowerCase())) {
      throw new Error(`已存在同名渠道:${clean}`)
    }
    const entry: CustomServiceType = {
      id,
      name: clean,
      createdAt: new Date().toISOString(),
    }
    state.customServiceTypes.push(entry)
    return entry
  },
  /** 从注册表移除用户自定义渠道;若仍有 instance 在用,保留但提示。 */
  removeCustomServiceType(typeId: string): void {
    const inUse = state.services.some((s) => s.type === typeId)
    if (inUse) {
      throw new Error('该渠道仍有实例在使用,请先删除或迁移实例')
    }
    state.customServiceTypes = state.customServiceTypes.filter((t) => t.id !== typeId)
  },
  /** 返回内置 + 用户自定义合并后的 ServiceMeta 列表(只读)。 */
  getMergedServices(): ServiceMeta[] {
    return buildServices(state.customServiceTypes)
  },
  /**
   * 追加一条 OCR 翻译历史。新条目插到数组头部(时间倒序)。
   * 自动按 `translation.historyLimit` 截断,避免长期使用后数组无限增长。
   */
  addHistory(entry: Omit<OcrHistoryEntry, 'id'>): OcrHistoryEntry {
    const full: OcrHistoryEntry = { id: newHistoryId(), ...entry }
    state.ocrHistory.unshift(full)
    const limit = Math.max(1, state.translation.historyLimit || 500)
    if (state.ocrHistory.length > limit) {
      state.ocrHistory.length = limit
    }
    return full
  },
  /** 删除单条历史。无 id 匹配时静默 no-op。 */
  removeHistory(entryId: string): void {
    const idx = state.ocrHistory.findIndex((e) => e.id === entryId)
    if (idx >= 0) state.ocrHistory.splice(idx, 1)
  },
  /** 清空全部历史。Confirm 由 UI 弹,这里只负责执行。 */
  clearHistory(): void {
    state.ocrHistory = []
  },
})
