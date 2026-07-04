import { reactive, watch } from 'vue'
import type {
  AppSettings,
  CustomServiceType,
  OcrHistoryEntry,
  ServiceId,
  ServiceInstance,
  ServiceMeta,
} from '../types'
import { BUILTIN_SERVICES, buildServices, DEFAULT_PROMPTS } from '../tokens'
import { projectToAppConfig, validateConfig, validateShortcutBindings } from '@/lib/config'
import type { Provider } from '@/types/config'
import { invokeSaveAppConfig, isTauriReady } from '@/lib/tauri'
import { toast } from '@/lib/toast'

const STORAGE_KEY = 'app:settings:v1'
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

/** 按渠道 type 返回后端默认 baseUrl（与 src-tauri/src/core/config/types.rs from_env 一致）。 */
const defaultEndpointFor = (type: ServiceId): string => {
  switch (type) {
    case 'openai':
    case 'custom':
      return 'https://api.openai.com/v1'
    case 'claude':
      return 'https://api.anthropic.com'
    default:
      return ''
  }
}

const defaultInstanceFor = (type: ServiceId, name: string): ServiceInstance => {
  const meta = BUILTIN_SERVICES.find((s) => s.id === type)
  return {
    id: newInstanceId(),
    type,
    name,
    enabled: true,
    apiKey: '',
    model: meta?.defaultModel ?? '',
    endpoint: defaultEndpointFor(type),
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

/** 首启仅 seed openai + claude 两个实例（精简后其余渠道由用户自行添加）。 */
const seedInstances = (): ServiceInstance[] =>
  ['openai', 'claude']
    .map((id) => BUILTIN_SERVICES.find((s) => s.id === id))
    .filter((m): m is ServiceMeta => !!m)
    .map((svc) => defaultInstanceFor(svc.id, svc.name))

/**
 * 默认 OCR 历史样本,首次启动(无 localStorage)时展示。
 * 覆盖"今天/昨天/本周/更早"四个时间桶,每条都是真实场景里能识别的常见英文/日文/韩文 UI 文案。
 * 时间戳基于当前时刻偏移,刷新后时间仍合理。
 */
const seedOcrHistory = (): OcrHistoryEntry[] => {
  const now = Date.now()
  const min = 60 * 1000
  const h = 60 * min
  const d = 24 * h
  return [
    {
      id: newHistoryId(),
      timestamp: new Date(now - 8 * min).toISOString(),
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'Save Changes',
      translation: '保存更改',
    },
    {
      id: newHistoryId(),
      timestamp: new Date(now - 25 * min).toISOString(),
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'Press any key to continue',
      translation: '按任意键继续',
    },
    {
      id: newHistoryId(),
      timestamp: new Date(now - 1.5 * h).toISOString(),
      sourceLang: 'ja',
      targetLang: 'zh-CN',
      source: '設定を保存しました',
      translation: '设置已保存',
    },
    {
      id: newHistoryId(),
      timestamp: new Date(now - 3.2 * h).toISOString(),
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: '404 - Page Not Found',
      translation: '404 - 页面未找到',
    },
    {
      id: newHistoryId(),
      timestamp: new Date(now - 1 * d - 2 * h).toISOString(),
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'The Federal Reserve raised interest rates by 25 basis points yesterday.',
      translation: '美联储昨日将利率上调了 25 个基点。',
    },
    {
      id: newHistoryId(),
      timestamp: new Date(now - 1 * d - 5 * h).toISOString(),
      sourceLang: 'ko',
      targetLang: 'zh-CN',
      source: '파일을 저장하시겠습니까?',
      translation: '您要保存文件吗?',
    },
    {
      id: newHistoryId(),
      timestamp: new Date(now - 3 * d).toISOString(),
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'TODO: refactor this function for better readability',
      translation: '待办:重构此函数以提高可读性',
    },
    {
      id: newHistoryId(),
      timestamp: new Date(now - 5 * d).toISOString(),
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'Push to open',
      translation: '推门开',
    },
    {
      id: newHistoryId(),
      timestamp: new Date(now - 12 * d).toISOString(),
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'The cake is a lie.',
      translation: '蛋糕是个谎言。',
    },
  ]
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
      defaultTargetLang: '中文',
      defaultServiceInstanceId: instances[0]?.id ?? '',
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
          keys: 'Alt+T',
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
          keys: 'Alt+O',
        },
        {
          id: 'word-lookup',
          label: '取词翻译',
          description: '光标悬停在词语上时弹出翻译结果。',
          keys: '',
        },
        {
          id: 'show-window',
          label: '显示/隐藏主窗口',
          description: '快速唤起或隐藏本应用的主窗口。',
          keys: 'Ctrl+Shift+Space',
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
        // 旧字段名 `defaultService` 兼容:把 ServiceId 映射到该类型首个 instance.id
        defaultServiceInstanceId:
          (parsed.translation as { defaultServiceInstanceId?: string } | undefined)
            ?.defaultServiceInstanceId ??
          ((parsed.translation as { defaultService?: ServiceId } | undefined)?.defaultService
            ? services.find(
                (s) =>
                  s.type ===
                  (parsed.translation as { defaultService?: ServiceId }).defaultService,
              )?.id ?? defaults.translation.defaultServiceInstanceId
            : defaults.translation.defaultServiceInstanceId),
      },
      shortcut: {
        bindings:
          parsed.shortcut?.bindings?.map((b) => {
            const def = defaults.shortcut.bindings.find((d) => d.id === b.id)
            return { ...def, ...b }
          }) ?? defaults.shortcut.bindings,
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

/** 内存持有上次成功保存的 provider，供非支持类型实例 fallback（spec §4.3）。首次为 openai-compatible。 */
let lastSavedProvider: Provider = 'openai-compatible'

const dirty = reactive({ value: false })
const baseline = JSON.parse(JSON.stringify(state)) as AppSettings

/**
 * 把状态序列化为 stable 字符串,排除 ocrHistory(它是"持久化数据"而非"待保存设置",
 * 不应触发 footer 的"放弃/保存"按钮)。
 */
const serializeForDirty = (s: AppSettings): string =>
  JSON.stringify({ ...s, ocrHistory: undefined })

const markDirty = (): void => {
  dirty.value = serializeForDirty(state) !== serializeForDirty(baseline)
}

const clearShortcutErrors = (): void => {
  for (const binding of state.shortcut.bindings) {
    binding.error = undefined
  }
}

const applyShortcutErrors = (errors: Record<string, string>): void => {
  for (const binding of state.shortcut.bindings) {
    binding.error = errors[binding.id]
  }
}

const applyBackendShortcutError = (error: unknown): string | null => {
  if (!error || typeof error !== 'object') return null
  const payload = error as { id?: unknown; message?: unknown }
  if (typeof payload.message !== 'string') return null

  if (typeof payload.id === 'string' && payload.id) {
    const binding = state.shortcut.bindings.find((item) => item.id === payload.id)
    if (binding) binding.error = payload.message
  }

  return payload.message
}

watch(state, markDirty, { deep: true })

watch(
  state,
  (next) => {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
  },
  { deep: true },
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
  async save(): Promise<void> {
    clearShortcutErrors()

    const shortcutErrors = validateShortcutBindings(state.shortcut.bindings)
    if (Object.keys(shortcutErrors).length > 0) {
      applyShortcutErrors(shortcutErrors)
      toast.error('保存失败', '请先解决重复快捷键')
      return
    }

    const { config, unsupported, unsupportedName } = projectToAppConfig(state, lastSavedProvider)
    const err = validateConfig(config)
    if (err) {
      toast.error('保存失败', err)
      return
    }
    if (isTauriReady()) {
      try {
        await invokeSaveAppConfig(config)
        lastSavedProvider = config.provider
        Object.assign(baseline, JSON.parse(JSON.stringify(state)))
        dirty.value = false
        if (unsupported) {
          toast.info('已本地保存', `默认服务「${unsupportedName}」暂未接入后端，仅本地保存`)
        } else {
          toast.success('配置已保存')
        }
      } catch (e) {
        const shortcutMessage = applyBackendShortcutError(e)
        toast.error('保存失败', shortcutMessage ?? String(e))
      }
    } else {
      Object.assign(baseline, JSON.parse(JSON.stringify(state)))
      dirty.value = false
      toast.info('Tauri 未就绪，仅本地保存')
    }
  },
  reset(): void {
    const defaults = buildDefaults()
    Object.assign(state, defaults)
  },
  discard(): void {
    Object.assign(state, JSON.parse(JSON.stringify(baseline)))
  },
  /** 在 services 数组末尾添加一条新实例并返回它;若 defaultService 仍为空,自动指向新实例。 */
  addService(type: ServiceId): ServiceInstance {
    const inst = defaultInstanceFor(type, nextDefaultName(type))
    state.services.push(inst)
    if (!state.translation.defaultServiceInstanceId) {
      state.translation.defaultServiceInstanceId = inst.id
    }
    return inst
  },
  removeService(instanceId: string): void {
    const idx = state.services.findIndex((s) => s.id === instanceId)
    if (idx < 0) return
    state.services.splice(idx, 1)
    if (state.translation.defaultServiceInstanceId === instanceId) {
      state.translation.defaultServiceInstanceId = state.services[0]?.id ?? ''
    }
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
