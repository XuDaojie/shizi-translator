import type { ServiceProtocolId } from '@/types/config';

export type ThemeMode = 'light' | 'dark' | 'system'
export type UILanguage = 'auto' | 'zh-CN' | 'zh-TW' | 'en-US' | 'ja-JP' | 'ko-KR' | 'fr-FR' | 'de-DE' | 'es-ES' | (string & {})
export type UpdateChannel = 'stable' | 'beta'
export type LogLevel = 'error' | 'warn' | 'info' | 'debug'

export interface GeneralSettings {
  /** 开机自启（已接后端 launchAtLogin → Windows Run）。 */
  launchAtLogin: boolean
  /** 主翻译弹窗是否预创建（后端已实现，接 save_app_config.popupPrecreate）。 */
  popupPrecreate: boolean
  /** 截图 OCR overlay 窗口是否预创建（后端已实现，接 save_app_config.overlayPrecreate）。 */
  overlayPrecreate: boolean
  theme: ThemeMode
  language: UILanguage
  updateChannel: UpdateChannel
  autoCheckUpdate: boolean
}

export interface AdvancedSettings {
  logLevel: LogLevel
  betaLookup: boolean
  betaVoice: boolean
  /** 是否收集匿名使用统计（后端已实现，接 save_app_config.collectUsage）。 */
  collectUsage: boolean
}

export type BuiltinServiceId =
  | 'openai'
  | 'deepseek'
  | 'claude'
  | 'microsoft'
  | 'gemini'
  | 'deepl'
  | 'google'
  | 'baidu'
  | 'youdao'
  | 'tencent'
  | 'volcengine'
  | 'iflytek'
  | 'zhipu'
  | 'moonshot'
  | 'siliconflow'
  | 'custom'

/** ServiceId 联合 = 内置 id 字符串 + 用户自定义 id 字符串(运行时收窄)。 */
export type ServiceId = BuiltinServiceId | (string & {})

/**
 * 用户在「服务」面板里手动新建的渠道类型(在右侧详情区输入"渠道名"时自动注册)。
 * 持久化到 settings.customServiceTypes,跨刷新保留。
 */
export interface CustomServiceType {
  /** id 形如 `custom_<slug>_<随机>`,作为 ServiceId 之一参与联合类型。 */
  id: string
  /** 渠道显示名(同时作为 type 的可读形式)。 */
  name: string
  /** ISO 时间戳,用于描述里"由用户在 {日期} 创建"。 */
  createdAt: string
}

/**
 * 一个"服务实例"= 一个 (渠道, Key, 模型) 三元组。
 * 同一渠道可创建多个实例,例如多个 OpenAI Key / 不同 region / 内部代理。
 */
export interface ServiceInstance {
  /** 实例唯一 id(用于 React key、store 查询、翻译默认服务选择)。 */
  id: string
  /** 渠道类型(决定 UI 显隐、默认 endpoint、是否能拉取模型等)。 */
  type: ServiceId
  /** 用户可编辑的显示名,默认 `${渠道名} N`。 */
  name: string
  enabled: boolean
  protocol: ServiceProtocolId
  apiKey: string
  model: string
  endpoint: string
  note: string
  /**
   * 用户主动从服务商拉取到的模型列表,用于在 combo 中作为候选值。
   * 与 `ServiceMeta.models`(静态内置候选)合并,共同出现在下拉中。
   */
  pulledModels: string[]
  /** Key 校验状态(由父组件驱动,目前为 mock)。空值时输入框不显状态。 */
  keyStatus: 'idle' | 'validating' | 'valid' | 'invalid'
  /**
   * 思维链长度,仅对 LLM 渠道生效(`ServiceMeta.category === 'llm'`)。
   * `off` 表示不启用思维链(默认),其余为推理深度档位。
   * UI 在非 LLM 渠道(机器翻译)上隐藏该字段。
   */
  chainOfThought: 'off' | 'short' | 'medium' | 'long'
  /**
   * 系统提示词,仅对 LLM 渠道生效。空字符串表示使用默认提示词。
   * 真实后端在每次对话前把该值作为 system role 发送给模型。
   */
  systemPrompt: string
  /**
   * 翻译提示词模板,仅对 LLM 渠道生效。支持占位符:
   * - `{source_lang}` 源语言显示名
   * - `{target_lang}` 目标语言显示名
   * - `{text}` 待翻译文本
   * 空字符串表示使用默认模板。
   */
  translationPrompt: string
  /**
   * 反思提示词,仅对 LLM 渠道生效。仅当 `reflectionEnabled` 为 true 时
   * 后端才会用该值对译后结果做一次自检并改进。
   */
  reflectionPrompt: string
  /** 是否启用反思环节(译后让模型自己审视并改进译文)。默认关闭。 */
  reflectionEnabled: boolean
}

export interface TranslationSettings {
  defaultSourceLang: string
  defaultTargetLang: string
  autoCopy: boolean
  restoreClipboard: boolean
  autoPaste: boolean
  showPhonetic: boolean
  showAlternatives: boolean
  autoDetect: boolean
  wordLookupDelay: number
  historyLimit: number
}

export interface ShortcutBinding {
  id: string
  label: string
  description: string
  keys: string
  /**
   * 绑定失败时的原因(系统占用、冲突等)。未设置表示当前快捷键可用。
   * 由后台注册接口返回,UI 据此渲染红边 + 错误说明。
   */
  error?: string
}

export interface ShortcutSettings {
  bindings: ShortcutBinding[]
}

export type ServiceProtocolMeta = {
  id: ServiceProtocolId
  label: string
  defaultEndpoint: string
  defaultModel: string
  editableEndpoint: boolean
  status: 'available' | 'planned'
}

export type OcrDetailKind = 'system' | 'vision-llm'

export type BuiltinOcrServiceId =
  | 'windows-media-ocr'
  | 'openai-vision'
  | 'claude-vision'
  | 'gemini-vision'
  | 'zhipu-vl'
  | 'siliconflow-vision'
  | 'moonshot-vision'
  | 'openai-compatible-vision'

/** OcrServiceId 联合 = 内置 id + 用户自定义 id（运行时收窄）。 */
export type OcrServiceId = BuiltinOcrServiceId | (string & {})

export type OcrServiceMeta = {
  id: OcrServiceId
  name: string
  description: string
  detail?: string
  builtin: boolean
  keyRequired: boolean
  canDisable: boolean
  canDelete: boolean
  multiInstance?: boolean
  protocol?: string
  /** 配置用协议 id，供 Key 校验 / 拉模型复用翻译 probe。 */
  protocolId?: 'openai_chat' | 'claude_messages'
  apiBaseUrl?: string
  docsUrl?: string
  apiKeyUrl?: string
  needsEndpoint?: boolean
  hasModelApi?: boolean
  defaultModel?: string
  models?: string[]
  iconifyId?: string
  detailKind: OcrDetailKind
  group: 'system' | 'vision'
  /**
   * 运行时是否支持作为截图 OCR 引擎启用。
   * 缺省视为 true；false 时 UI/store 拒绝启用（如 Claude 视觉本版本不支持）。
   */
  runtimeSupported?: boolean
}

export type OcrServiceInstance = {
  id: string
  type: OcrServiceId
  name: string
  enabled: boolean
  apiKey: string
  endpoint: string
  note: string
  keyStatus: 'idle' | 'validating' | 'valid' | 'invalid'
  preferredLang: string
  model: string
  pulledModels: string[]
  ocrPrompt: string
}

export type AppSettings = {
  general: GeneralSettings
  translation: TranslationSettings
  shortcut: ShortcutSettings
  /** 服务实例数组,允许同一渠道多个实例。 */
  services: ServiceInstance[]
  /** OCR 服务实例数组（system + 视觉 LLM）。 */
  ocrServices: OcrServiceInstance[]
  /** 用户在「服务」面板里手动新建的渠道类型(右侧详情区输入"渠道名"时注册)。 */
  customServiceTypes: CustomServiceType[]
  advanced: AdvancedSettings
}

export type ServiceMeta = {
  id: ServiceId
  name: string
  description: string
  builtin: boolean
  defaultModel?: string
  models?: string[]
  needsEndpoint?: boolean
  /** 该服务商是否提供 /models 类端点,允许在设置页"拉取模型"。 */
  hasModelApi?: boolean
  /**
   * Iconify 图标 id(格式: `<collection>:<icon>`,如 `simple-icons:openai`)。
   * 留空时由 `getServiceLogoSrc` 查本地 lobe-icons SVG，再 Lucide fallback。
   */
  iconifyId?: string
  /**
   * 渠道类别:
   * - `llm`:大模型(OpenAI / Claude / Gemini / DeepSeek / Moonshot / 智谱 / 自定义兼容等)
   *   — 显示「思维链长度」选项,允许用户控制推理深度
   * - `ml`:机器翻译(DeepL / Google 翻译 / 百度 / 有道 / 腾讯 / 火山 / 讯飞)
   *   — 隐藏「思维链长度」选项(无推理过程可言)
   */
  category: 'llm' | 'ml'
  /** 是否需要用户自行提供 API Key。true = 「需要密钥」,false = 「内置」(系统自带/本地)。 */
  keyRequired: boolean
  officialEndpoint?: string
  protocols: ServiceProtocolMeta[]
  /** 官方文档外链；有则详情 Header 显示「查看文档」。 */
  docsUrl?: string
  /** API Key 申请页；有则 Header / 缺 Key 警告显示「申请 API Key」。 */
  apiKeyUrl?: string
}
