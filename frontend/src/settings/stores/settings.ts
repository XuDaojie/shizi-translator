import { reactive, readonly, ref, watch } from 'vue'
import type {
  AppSettings,
  CustomServiceType,
  LogLevel,
  OcrServiceId,
  OcrServiceInstance,
  OcrServiceMeta,
  ServiceId,
  ServiceInstance,
  ServiceMeta,
} from '../types'
import type { AppConfig, OcrServiceInstanceConfig, ServiceInstanceConfig } from '@/types/config'
import {
  BUILTIN_OCR_SERVICES,
  BUILTIN_SERVICES,
  buildServices,
  DEFAULT_PROMPTS,
  ocrServiceById,
} from '../tokens'
import { projectToAppConfig, validateConfig } from '@/lib/config'
import {
  invokeGetAppConfig,
  invokeGetInterfaceLanguageSnapshot,
  invokeGetShortcutConflicts,
  invokeOpenLanguagePackDirectory,
  invokeRefreshInterfaceLanguages,
  invokeSaveAppConfig,
  isTauriReady,
  type InterfaceLanguageSnapshot,
  type LanguageMeta,
  type LanguagePackError,
  type ShortcutConflict,
} from '@/lib/tauri'
import { toast } from '@/lib/toast'
import { createLogger } from '@public/logger.js'
import { t } from '@/i18n'

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

const defaultOcrInstanceFor = (
  type: OcrServiceId,
  name: string,
  enabled = false,
): OcrServiceInstance => {
  const meta = ocrServiceById(type)
  return {
    id: newInstanceId(),
    type,
    name,
    enabled,
    apiKey: '',
    endpoint: meta?.apiBaseUrl ?? '',
    note: '',
    keyStatus: 'idle',
    preferredLang: '',
    model: meta?.defaultModel ?? '',
    pulledModels: [],
    ocrPrompt: '',
  }
}

/** 首启仅 seed Windows 媒体 OCR，强制启用。视觉实例由用户添加。 */
const seedOcrInstances = (): OcrServiceInstance[] => {
  const win = BUILTIN_OCR_SERVICES.find((s) => s.id === 'windows-media-ocr')
  if (!win) return []
  return [defaultOcrInstanceFor(win.id, win.name, true)]
}

/**
 * 保证含 Windows 行；零 enabled → 开 Windows；多 enabled → 只留第一个。
 * 若唯一 enabled 为 runtimeSupported===false → 关它并开 Windows。
 */
export const normalizeOcrList = (list: OcrServiceInstance[]): OcrServiceInstance[] => {
  let next = [...list]
  // 缺 Windows 时补行；enabled 先 false，由下方「零 enabled → 开 Windows」分支决定是否打开，
  // 避免 seed 的 true 与已有视觉 enabled 形成「多开」后误删视觉。
  if (!next.some((s) => s.type === 'windows-media-ocr')) {
    next = [
      ...seedOcrInstances().map((s) => ({ ...s, enabled: false })),
      ...next,
    ]
  }
  let enabledIdxs = next
    .map((s, i) => (s.enabled ? i : -1))
    .filter((i) => i >= 0)
  if (enabledIdxs.length === 0) {
    return next.map((s) =>
      s.type === 'windows-media-ocr' ? { ...s, enabled: true } : { ...s, enabled: false },
    )
  }
  if (enabledIdxs.length > 1) {
    const keep = enabledIdxs[0]
    next = next.map((s, i) => ({ ...s, enabled: i === keep }))
    enabledIdxs = [keep]
  }
  if (enabledIdxs.length === 1) {
    const only = next[enabledIdxs[0]]
    const meta = ocrServiceById(only.type)
    if (meta?.runtimeSupported === false) {
      return next.map((s) => ({
        ...s,
        enabled: s.type === 'windows-media-ocr',
      }))
    }
  }
  return next
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
      language: 'auto',
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
          keys: 'Alt+S',
        },
        {
          id: 'ocr-recognize',
          label: '文字识别',
          description: '打开文字识别窗口并框选屏幕区域识别文字。',
          keys: 'Alt+O',
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
    ocrServices: seedOcrInstances(),
    customServiceTypes: [],
    advanced: {
      logLevel: 'info',
      betaLookup: false,
      betaVoice: false,
      collectUsage: true,
    },
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

/** 后端 OCR 配置合并：核心字段以后端为准；keyStatus/pulledModels/note 保留前端；末尾 normalize。 */
export const mergeBackendIntoOcrServices = (
  local: OcrServiceInstance[],
  backend: OcrServiceInstanceConfig[],
): OcrServiceInstance[] => {
  if (!backend.length) {
    return local.length ? local : seedOcrInstances()
  }
  const localById = new Map(local.map((s) => [s.id, s]))
  const merged: OcrServiceInstance[] = backend.map((b) => {
    const existing = localById.get(b.id)
    if (!existing) {
      return {
        id: b.id,
        type: b.serviceType as OcrServiceId,
        name: b.name,
        enabled: b.enabled,
        apiKey: b.apiKey ?? '',
        endpoint: b.endpoint,
        note: '',
        keyStatus: 'idle' as const,
        preferredLang: b.preferredLang ?? '',
        model: b.model,
        pulledModels: [],
        ocrPrompt: b.ocrPrompt ?? '',
      }
    }
    return {
      ...existing,
      name: b.name,
      enabled: b.enabled,
      apiKey: b.apiKey ?? '',
      endpoint: b.endpoint,
      model: b.model,
      preferredLang: b.preferredLang ?? '',
      ocrPrompt: b.ocrPrompt ?? '',
      type: b.serviceType as OcrServiceId,
    }
  })
  return normalizeOcrList(merged)
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
      ocrServices: (() => {
        const incoming = Array.isArray(parsed.ocrServices) ? parsed.ocrServices : []
        if (incoming.length === 0) return seedOcrInstances()
        const hydrated = incoming.map((s) => ({
          ...s,
          keyStatus: s.keyStatus ?? 'idle',
          preferredLang: s.preferredLang ?? '',
          model: s.model ?? '',
          pulledModels: s.pulledModels ?? [],
          ocrPrompt: s.ocrPrompt ?? '',
          note: s.note ?? '',
          apiKey: s.apiKey ?? '',
          endpoint: s.endpoint ?? '',
        }))
        return normalizeOcrList(hydrated)
      })(),
      customServiceTypes: parsed.customServiceTypes ?? [],
      advanced: { ...defaults.advanced, ...parsed.advanced },
    }
  } catch {
    return buildDefaults()
  }
}

const state = reactive<AppSettings>(loadFromStorage())
const interfaceLanguages = ref<LanguageMeta[]>([])
const interfaceLanguageErrors = ref<LanguagePackError[]>([])
const interfaceLanguagesRefreshing = ref(false)

const applyInterfaceLanguageSnapshot = (snapshot: InterfaceLanguageSnapshot): void => {
  interfaceLanguages.value = snapshot.languages.filter(({ locale }) => locale !== 'auto')
  interfaceLanguageErrors.value = snapshot.errors
  if (state.general.language === 'auto' || interfaceLanguages.value.some(({ locale }) => locale === state.general.language)) return
  const locales = new Set(interfaceLanguages.value.map(({ locale }) => locale))
  state.general.language = locales.has(snapshot.configuredLocale)
    ? snapshot.configuredLocale
    : locales.has(snapshot.locale) ? snapshot.locale : 'auto'
}

const dirty = reactive({ value: false })
const saveStatus = reactive<{ value: 'idle' | 'saved' | 'saving' | 'error' }>({ value: 'idle' })
const baseline = JSON.parse(JSON.stringify(state)) as AppSettings
let autoSaveTimer: ReturnType<typeof setTimeout> | undefined
let saveStatusIdleTimer: ReturnType<typeof setTimeout> | undefined
let persistQueue: Promise<void> = Promise.resolve()
let syncingFromBackend = false
let latestLanguageRefreshRequest = 0
let latestInterfaceLanguageChange = 0
let syncFromBackendPromise: Promise<void> | null = null
const backendSyncConflict = new Error(t('settings.status.saveFailed'))

const readConsistentBackendState = async (): Promise<{
  config: AppConfig
  snapshot: InterfaceLanguageSnapshot
}> => {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const before = await invokeGetInterfaceLanguageSnapshot()
    const config = await invokeGetAppConfig()
    const after = await invokeGetInterfaceLanguageSnapshot()
    if (
      before.revision === after.revision
      && config.interfaceLanguage === after.configuredLocale
    ) return { config, snapshot: after }
  }
  throw backendSyncConflict
}

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

/** 把状态序列化为 stable 字符串,排除运行时字段,避免误触发 footer 的"放弃/保存"按钮。 */
const serializeForDirty = (s: AppSettings): string =>
  JSON.stringify({
    ...s,
    services: s.services.map((service) => ({ ...service, keyStatus: 'idle' })),
    ocrServices: s.ocrServices.map((ocr) => ({ ...ocr, keyStatus: 'idle' })),
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

const persist = (notify = false): Promise<void> => {
  const snapshot = cloneSettings(state)
  const config = projectToAppConfig(snapshot)
  const err = validateConfig(config)
  const run = async (): Promise<void> => {
    if (err) {
      saveStatus.value = 'error'
      toast.error(t('settings.toast.saveFailed'), err)
      logger.warn('配置校验失败', err)
      return
    }
    try {
      if (isTauriReady()) {
        await invokeSaveAppConfig(config)
        if (notify) toast.success(t('settings.toast.saved'))
      } else if (notify) {
        toast.info(t('settings.toast.saved'), t('settings.status.localPreference'))
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
      toast.error(t('settings.toast.saveFailed'), String(e))
      logger.error('保存配置失败', String(e))
    }
  }
  const queued = persistQueue.then(run, run)
  persistQueue = queued
  return queued
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

/** OCR 同 type 多实例命名：与翻译渠道同名（如 `OpenAI`、`OpenAI 2`） */
const nextOcrDefaultName = (type: OcrServiceId): string => {
  const meta = ocrServiceById(type)
  const base = meta?.name ?? type
  const sameType = state.ocrServices.filter((s) => s.type === type).length
  return sameType === 0 ? base : `${base} ${sameType + 1}`
}

export const useSettings = () => ({
  state,
  dirty,
  saveStatus,
  interfaceLanguages: readonly(interfaceLanguages),
  interfaceLanguageErrors: readonly(interfaceLanguageErrors),
  interfaceLanguagesRefreshing: readonly(interfaceLanguagesRefreshing),
  async refreshInterfaceLanguages(): Promise<void> {
    const requestId = ++latestLanguageRefreshRequest
    interfaceLanguagesRefreshing.value = true
    try {
      const snapshot = await invokeRefreshInterfaceLanguages()
      if (requestId === latestLanguageRefreshRequest) applyInterfaceLanguageSnapshot(snapshot)
    } catch (error) {
      if (requestId === latestLanguageRefreshRequest) throw error
    } finally {
      if (requestId === latestLanguageRefreshRequest) interfaceLanguagesRefreshing.value = false
    }
  },
  openLanguagePackDirectory: invokeOpenLanguagePackDirectory,
  async setInterfaceLanguage(value: string): Promise<void> {
    if (state.general.language === value) return Promise.resolve()
    const changeId = ++latestInterfaceLanguageChange
    const syncing = syncFromBackendPromise
    state.general.language = value
    if (autoSaveTimer) clearTimeout(autoSaveTimer)
    autoSaveTimer = undefined
    if (syncing) {
      await syncing
      if (changeId !== latestInterfaceLanguageChange) return
      state.general.language = value
      if (autoSaveTimer) clearTimeout(autoSaveTimer)
      autoSaveTimer = undefined
    }
    await persist()
  },
  async save(): Promise<void> {
    if (autoSaveTimer) clearTimeout(autoSaveTimer)
    saveStatus.value = 'saving'
    await persist(true)
  },
  /** 启动时从后端 config.json 同步：后端空则推前端覆盖，后端非空则按 id 合并。失败静默降级。 */
  syncFromBackend(): Promise<void> {
    if (!isTauriReady()) return Promise.resolve()
    if (syncFromBackendPromise) return syncFromBackendPromise
    const execution = (async (): Promise<void> => {
    if (autoSaveTimer) clearTimeout(autoSaveTimer)
    autoSaveTimer = undefined
    syncingFromBackend = true
    let resumeAutoSave = true
    let pushFailed = false
    try {
      let backend: AppConfig
      let languageSnapshot: InterfaceLanguageSnapshot
      try {
        ({ config: backend, snapshot: languageSnapshot } = await readConsistentBackendState())
      } catch (error) {
        if (error === backendSyncConflict) {
          resumeAutoSave = false
          saveStatus.value = 'error'
          throw error
        }
        logger.warn('从后端同步配置失败')
        return
      }
      if (!backend.services || backend.services.length === 0) {
        applyInterfaceLanguageSnapshot(languageSnapshot)
        const pushed = cloneSettings(state)
        try {
          await invokeSaveAppConfig(projectToAppConfig(pushed))
          commitBaseline(pushed)
          saveStatus.value = 'idle'
        } catch {
          const changedDuringPush = serializeForDirty(state) !== serializeForDirty(pushed)
          resumeAutoSave = changedDuringPush
          pushFailed = !changedDuringPush
          logger.warn('推送配置到后端失败')
        }
        await refreshShortcutConflicts()
        return
      }
      state.services = mergeBackendIntoServices(state.services, backend.services)
      state.ocrServices = mergeBackendIntoOcrServices(
        state.ocrServices,
        backend.ocrServices ?? [],
      )
      state.general.language = backend.interfaceLanguage
      state.general.updateChannel =
        backend.updateChannel === 'beta' ? 'beta' : 'stable'
      state.general.autoCheckUpdate =
        backend.autoCheckUpdate ?? state.general.autoCheckUpdate
      applyInterfaceLanguageSnapshot(languageSnapshot)
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
      state.advanced.logLevel = applyBackendLogLevel(state.advanced.logLevel, backend.logLevel)
      logger.setLevel(state.advanced.logLevel)
      const synced = cloneSettings(state)
      await refreshShortcutConflicts()
      commitBaseline(synced)
      saveStatus.value = 'idle'
    } finally {
      syncingFromBackend = false
      if (pushFailed) {
        dirty.value = true
        saveStatus.value = 'error'
      } else if (resumeAutoSave) markDirty()
      if (resumeAutoSave && dirty.value) {
        saveStatus.value = 'saving'
        autoSaveTimer = setTimeout(() => void persist(), 300)
      }
    }
    })()
    const flight = execution.finally(() => {
      if (syncFromBackendPromise === flight) syncFromBackendPromise = null
    })
    syncFromBackendPromise = flight
    return flight
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
    if (!clean) throw new Error(t('settings.error.channelNameRequired'))
    const id = newCustomTypeId(clean)
    const all = buildServices(state.customServiceTypes)
    if (all.some((s) => s.name.toLowerCase() === clean.toLowerCase())) {
      throw new Error(t('settings.error.duplicateChannel', { name: clean }))
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
      throw new Error(t('settings.error.channelInUse'))
    }
    state.customServiceTypes = state.customServiceTypes.filter((t) => t.id !== typeId)
  },
  /** 返回内置 + 用户自定义合并后的 ServiceMeta 列表(只读)。 */
  getMergedServices(): ServiceMeta[] {
    return buildServices(state.customServiceTypes)
  },
  /**
   * 添加 OCR 实例。`windows-media-ocr` 已存在则返回已有；
   * 视觉实例默认 enabled=false，允许多开且不互斥。
   */
  addOcrService(type: OcrServiceId): OcrServiceInstance {
    if (type === 'windows-media-ocr') {
      const existing = state.ocrServices.find((s) => s.type === 'windows-media-ocr')
      if (existing) return existing
    }
    const inst = defaultOcrInstanceFor(type, nextOcrDefaultName(type), false)
    state.ocrServices.push(inst)
    return inst
  },
  /** Windows 媒体 OCR 不可删除；其它实例按 id 删除。删除后无 enabled 则 normalize 开 Windows。 */
  removeOcrService(instanceId: string): void {
    const inst = state.ocrServices.find((s) => s.id === instanceId)
    if (!inst || inst.type === 'windows-media-ocr') return
    const idx = state.ocrServices.findIndex((s) => s.id === instanceId)
    if (idx < 0) return
    state.ocrServices.splice(idx, 1)
    if (!state.ocrServices.some((s) => s.enabled)) {
      state.ocrServices = normalizeOcrList(state.ocrServices)
    }
  },
  /**
   * OCR 互斥开关：启用时仅该项 on；runtimeSupported===false 拒绝启用；
   * 关闭唯一项时：Windows 拒绝；视觉则关视觉并开 Windows。
   */
  setOcrEnabled(instanceId: string, enabled: boolean): void {
    const inst = state.ocrServices.find((s) => s.id === instanceId)
    if (!inst) return
    const meta = ocrServiceById(inst.type)
    if (enabled && meta?.runtimeSupported === false) {
      return
    }
    if (enabled) {
      state.ocrServices = state.ocrServices.map((s) => ({
        ...s,
        enabled: s.id === instanceId,
      }))
      return
    }
    const enabledCount = state.ocrServices.filter((s) => s.enabled).length
    const isOnly = inst.enabled && enabledCount === 1
    if (!isOnly) {
      inst.enabled = false
      return
    }
    if (inst.type === 'windows-media-ocr') {
      inst.enabled = true
      return
    }
    inst.enabled = false
    const win = state.ocrServices.find((s) => s.type === 'windows-media-ocr')
    if (win) win.enabled = true
    else state.ocrServices = normalizeOcrList(state.ocrServices)
  },
  /** Windows 不可重命名；其它实例改名。 */
  renameOcrService(instanceId: string, name: string): void {
    const inst = state.ocrServices.find((s) => s.id === instanceId)
    if (!inst || inst.type === 'windows-media-ocr') return
    inst.name = name.trim() || inst.name
  },
  findOcrInstance(instanceId: string): OcrServiceInstance | undefined {
    return state.ocrServices.find((s) => s.id === instanceId)
  },
  /** 内置 OCR 元数据列表（只读）。 */
  getMergedOcrServices(): OcrServiceMeta[] {
    return BUILTIN_OCR_SERVICES
  },
})
