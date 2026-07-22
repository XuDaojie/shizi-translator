import { Plug, WandSparkles } from '@lucide/vue'
import type { Component } from 'vue'
import type {
  CustomServiceType,
  OcrServiceId,
  OcrServiceMeta,
  ServiceId,
  ServiceMeta,
} from './types'

// ── 协议元数据 ────────────────────────────────────────────
const OPENAI_CHAT = (defaultEndpoint: string, defaultModel: string) => ({
  id: 'openai_chat' as const,
  label: 'OpenAI Chat',
  defaultEndpoint,
  defaultModel,
  editableEndpoint: true,
  status: 'available' as const,
})

const CLAUDE_MESSAGES = {
  id: 'claude_messages' as const,
  label: 'Claude Messages',
  defaultEndpoint: 'https://api.anthropic.com',
  defaultModel: 'claude-haiku-4-5',
  editableEndpoint: true,
  status: 'available' as const,
}

const MICROSOFT_EDGE = {
  id: 'microsoft_edge' as const,
  label: 'Edge 翻译',
  defaultEndpoint: 'https://edge.microsoft.com/translate/translatetext',
  defaultModel: '',
  editableEndpoint: false,
  status: 'available' as const,
}

/**
 * 厂商官方 logo 解析顺序：
 * 1. Iconify simple-icons（https://api.iconify.design/?prefix=simple-icons）
 * 2. 本地 vendored SVG（lobe-icons，见 service-logos.ts：智谱/硅基/火山/腾讯）
 * 3. Lucide `Plug` 兜底（有道、讯飞等仍无品牌图；自定义渠道用 WandSparkles）
 *
 * 命名约定：
 * - 优先 simple-icons 英文品牌条目
 * - 不使用相似品牌替代、不为每个服务单独挑不同 lucide 图标
 */
export const getServiceIconifyId = (id: ServiceId): string | undefined =>
  BUILTIN_SERVICES.find((s) => s.id === id)?.iconifyId

export { getServiceLogoSrc } from './service-logos'

export const getServiceLucideFallback = (_id: ServiceId): Component => Plug

export const BUILTIN_SERVICES: ServiceMeta[] = [
  {
    id: 'openai',
    name: 'OpenAI',
    description: 'GPT-4o / GPT-4o-mini 等通用模型，推理质量稳定。',
    builtin: true,
    defaultModel: 'gpt-4o-mini',
    models: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo'],
    hasModelApi: true,
    iconifyId: 'simple-icons:openai',
    category: 'llm',
    keyRequired: true,
    protocols: [OPENAI_CHAT('https://api.openai.com/v1', 'gpt-4o-mini')],
    docsUrl: 'https://developers.openai.com/api/docs',
    apiKeyUrl: 'https://platform.openai.com/api-keys',
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    description: '国产高性价比模型，长上下文表现优秀。',
    builtin: true,
    defaultModel: 'deepseek-chat',
    models: ['deepseek-chat', 'deepseek-reasoner'],
    hasModelApi: true,
    iconifyId: 'simple-icons:deepseek',
    category: 'llm',
    keyRequired: true,
    protocols: [OPENAI_CHAT('https://api.deepseek.com', 'deepseek-chat')],
    docsUrl: 'https://platform.deepseek.com/api-docs',
    apiKeyUrl: 'https://platform.deepseek.com/api_keys',
  },
  {
    id: 'claude',
    name: 'Claude',
    description: 'Anthropic Claude 系列，长文与写作更自然。',
    builtin: true,
    defaultModel: 'claude-haiku-4-5',
    models: ['claude-haiku-4-5', 'claude-3-5-sonnet-latest', 'claude-3-5-haiku-latest'],
    hasModelApi: true,
    iconifyId: 'simple-icons:anthropic',
    category: 'llm',
    keyRequired: true,
    protocols: [CLAUDE_MESSAGES],
    docsUrl: 'https://docs.anthropic.com',
    apiKeyUrl: 'https://console.anthropic.com/settings/keys',
  },
  {
    id: 'microsoft',
    name: '微软翻译',
    description: 'Edge 浏览器默认翻译引擎，免 Key，复用浏览器环境信息调用。',
    builtin: true,
    defaultModel: '',
    iconifyId: 'simple-icons:microsofttranslator',
    category: 'ml',
    keyRequired: false,
    protocols: [MICROSOFT_EDGE],
    docsUrl: 'https://learn.microsoft.com/azure/ai-services/translator',
  },
  {
    id: 'gemini',
    name: 'Gemini',
    description: 'Google Gemini 系列，多模态与多语言支持强。',
    builtin: true,
    defaultModel: 'gemini-1.5-flash',
    models: ['gemini-1.5-pro', 'gemini-1.5-flash', 'gemini-1.5-flash-8b'],
    hasModelApi: true,
    iconifyId: 'simple-icons:google',
    category: 'llm',
    keyRequired: true,
    // Google OpenAI 兼容端点：{base}/chat/completions
    protocols: [
      OPENAI_CHAT(
        'https://generativelanguage.googleapis.com/v1beta/openai',
        'gemini-1.5-flash',
      ),
    ],
    docsUrl: 'https://ai.google.dev/docs',
    apiKeyUrl: 'https://aistudio.google.com/apikey',
  },
  {
    id: 'deepl',
    name: 'DeepL',
    description: '欧洲语种翻译质量领先,适合学术与商务场景。',
    builtin: true,
    defaultModel: 'deepl',
    iconifyId: 'simple-icons:deepl',
    category: 'ml',
    keyRequired: true,
    protocols: [],
  },
  {
    id: 'google',
    name: 'Google 翻译',
    description: '免 Key 的通用翻译,适合轻量场景。',
    builtin: true,
    defaultModel: 'google',
    iconifyId: 'simple-icons:googletranslate',
    category: 'ml',
    keyRequired: false,
    protocols: [],
  },
  {
    id: 'baidu',
    name: '百度翻译',
    description: '中英语料库大,国内网络环境稳定。',
    builtin: true,
    defaultModel: 'baidu',
    iconifyId: 'simple-icons:baidu',
    category: 'ml',
    keyRequired: true,
    protocols: [],
  },
  {
    id: 'youdao',
    name: '有道翻译',
    description: '支持词典与例句,适合学习场景。',
    builtin: true,
    defaultModel: 'youdao',
    category: 'ml',
    keyRequired: true,
    protocols: [],
  },
  {
    id: 'tencent',
    name: '腾讯翻译君',
    description: '多语种均衡,API 配额灵活。',
    builtin: true,
    defaultModel: 'tencent',
    category: 'ml',
    keyRequired: true,
    protocols: [],
  },
  {
    id: 'volcengine',
    name: '火山翻译',
    description: '字节跳动旗下,小语种质量不错。',
    builtin: true,
    defaultModel: 'volcengine',
    hasModelApi: true,
    category: 'ml',
    keyRequired: true,
    protocols: [],
  },
  {
    id: 'iflytek',
    name: '科大讯飞',
    description: '中英语音与翻译结合,适合会议场景。',
    builtin: true,
    defaultModel: 'iflytek',
    category: 'ml',
    keyRequired: true,
    protocols: [],
  },
  {
    id: 'zhipu',
    name: '智谱 AI',
    description: 'GLM 系列，中文写作与编程能力稳定。',
    builtin: true,
    defaultModel: 'glm-4-flash',
    models: ['glm-4-plus', 'glm-4-air', 'glm-4-flash'],
    hasModelApi: true,
    category: 'llm',
    keyRequired: true,
    protocols: [OPENAI_CHAT('https://open.bigmodel.cn/api/paas/v4', 'glm-4-flash')],
    docsUrl: 'https://open.bigmodel.cn/dev/api',
    apiKeyUrl: 'https://open.bigmodel.cn/usercenter/apikeys',
  },
  {
    id: 'moonshot',
    name: '月之暗面',
    description: 'Moonshot 长上下文模型,适合长文档翻译。',
    builtin: true,
    defaultModel: 'moonshot-v1-128k',
    models: ['moonshot-v1-8k', 'moonshot-v1-32k', 'moonshot-v1-128k'],
    hasModelApi: true,
    iconifyId: 'simple-icons:moonshotai',
    category: 'llm',
    keyRequired: true,
    protocols: [OPENAI_CHAT('https://api.moonshot.cn/v1', 'moonshot-v1-128k')],
    docsUrl: 'https://platform.moonshot.cn/docs',
    apiKeyUrl: 'https://platform.moonshot.cn/console/api-keys',
  },
  {
    id: 'siliconflow',
    name: '硅基流动',
    description: '国产 AI 推理云,提供 Qwen/GLM/DeepSeek 等开源模型的 OpenAI 兼容 API。',
    builtin: true,
    defaultModel: 'Qwen/Qwen2.5-7B-Instruct',
    models: [
      'Qwen/Qwen2.5-7B-Instruct',
      'Qwen/Qwen2.5-14B-Instruct',
      'Qwen/Qwen2.5-32B-Instruct',
      'Qwen/Qwen2.5-72B-Instruct',
      'deepseek-ai/DeepSeek-V2.5',
      'deepseek-ai/DeepSeek-R1',
      'THUDM/glm-4-9b-chat',
    ],
    hasModelApi: true,
    category: 'llm',
    keyRequired: true,
    protocols: [OPENAI_CHAT('https://api.siliconflow.cn/v1', 'Qwen/Qwen2.5-7B-Instruct')],
    docsUrl: 'https://docs.siliconflow.cn',
    apiKeyUrl: 'https://cloud.siliconflow.cn/account/ak',
  },
  {
    id: 'custom',
    name: '自定义 OpenAI 兼容',
    description: '接入任意 OpenAI 兼容协议的端点(如本地 Ollama、Azure OpenAI 等)。',
    builtin: true,
    defaultModel: '',
    needsEndpoint: true,
    hasModelApi: true,
    category: 'llm',
    keyRequired: true,
    // 与 openai/deepseek/zhipu 共用 openai_chat 后端路径，用户自填 endpoint/model/key。
    protocols: [OPENAI_CHAT('http://localhost:11434/v1', '')],
    docsUrl: 'https://developers.openai.com/api/docs',
  },
]

export const serviceById = (id: ServiceId): ServiceMeta | undefined =>
  BUILTIN_SERVICES.find((s) => s.id === id)

/**
 * 把内置渠道 + 用户在「服务」面板里手动注册的渠道合并为完整的 ServiceMeta 列表。
 * 用户自定义 type 默认:
 * - iconifyId: undefined → Lucide 兜底
 * - defaultModel: '' (由用户在使用时填)
 * - needsEndpoint: false(用户自己决定)
 * - hasModelApi: false(走通用 OpenAI 兼容时,用户可改 instance.endpoint 接入)
 * - description: "由用户在 {日期} 创建的自定义渠道"
 */
export const buildServices = (customTypes: CustomServiceType[]): ServiceMeta[] => {
  const customs: ServiceMeta[] = customTypes.map((t) => ({
    id: t.id,
    name: t.name,
    description: `由用户创建的自定义渠道(${t.createdAt.slice(0, 10)})`,
    builtin: false,
    defaultModel: '',
    needsEndpoint: true,
    hasModelApi: true,
    category: 'llm',
    keyRequired: true,
    // 用户自建渠道同样走 OpenAI Chat 兼容协议（endpoint/model 由用户填写）。
    protocols: [OPENAI_CHAT('', '')],
  }))
  return [...BUILTIN_SERVICES, ...customs]
}

/** 用户自定义 type 的 Lucide 兜底(目前统一 WandSparkles,可后续按需扩展)。 */
export const LUCIDE_CUSTOM_FALLBACK: Component = WandSparkles

/**
 * LLM 渠道默认提示词,风格对标 Bob / OpenAI Translator。
 * 占位符(仅 translationPrompt 有效):
 * - {source_lang} 源语言显示名
 * - {target_lang} 目标语言显示名
 * - {text} 待翻译文本
 * 用户在设置页可覆盖;UI 旁提供"重置为默认"按钮。
 */
export const DEFAULT_PROMPTS = {
  system:
    '你是一位专业的翻译助手,擅长准确、自然地将文本从源语言翻译成目标语言。保持原文的语气、风格和专业术语的一致性;遇到专有名词、品牌名、代码片段保留原文;必须完整翻译全部内容,保留原文的换行、段落与列表结构,不得遗漏条目或提前结束;只输出翻译结果,不要附加解释。',
  translation:
    '请将以下文本从 {source_lang} 完整翻译成 {target_lang}（保留所有段落、换行与列表项,勿省略任何内容）。只输出翻译结果,不要添加任何解释或说明:\n\n{text}',
  reflection:
    '请审视上面的翻译,从以下角度检查并改进:\n1. 是否有不符合目标语言习惯的表达?\n2. 专业术语是否一致?\n3. 是否保持了原文的语气和风格?\n4. 是否有遗漏或错译?\n\n请直接输出改进后的最终翻译,不要附加说明。',
} as const

/** 通用视觉模型默认 OCR 提示；实例 ocrPrompt 为空且非 DeepSeek-OCR 时使用。 */
export const DEFAULT_OCR_PROMPT =
  '请识别图中全部文字，按阅读顺序完整输出。只输出文字，不要解释。'

/** DeepSeek-OCR 官方推荐默认任务句；实例 ocrPrompt 为空且模型为 DeepSeek-OCR 时使用。 */
export const DEFAULT_DEEPSEEK_OCR_PROMPT = 'Free OCR.'

/** 按模型选择空配置时的默认 OCR 提示（与后端 `effective_ocr_prompt` 对齐）。 */
export function defaultOcrPromptForModel(model: string): string {
  const m = model.toLowerCase()
  if (m.includes('deepseek-ocr') || m.includes('deepseek_ocr')) {
    return DEFAULT_DEEPSEEK_OCR_PROMPT
  }
  return DEFAULT_OCR_PROMPT
}

/**
 * 内置 OCR 服务元数据。
 * - system：Windows 媒体 OCR（可与视觉互斥切换；不可全关、不可删）
 * - vision：对应翻译侧已对接且具备多模态能力的 LLM（不含 DeepSeek / Edge / ML / 专用 OCR）
 * - runtimeSupported=false：本版本不可作为截图 OCR 引擎启用（如 Claude 视觉）
 */
export const BUILTIN_OCR_SERVICES: OcrServiceMeta[] = [
  {
    id: 'windows-media-ocr',
    name: 'Windows 媒体 OCR',
    description: 'Windows 10+ 系统自带 OCR，无需 API Key。',
    detail: '截图识别使用当前启用的文字识别服务；与视觉渠道互斥，仅一项生效。',
    builtin: true,
    keyRequired: false,
    canDisable: true,
    canDelete: false,
    detailKind: 'system',
    group: 'system',
    runtimeSupported: true,
  },
  // 渠道展示名与翻译服务 BUILTIN_SERVICES 对齐（不加「视觉」后缀；上下文已是文字识别）
  {
    id: 'openai-vision',
    name: 'OpenAI',
    description: 'GPT-4o 等多模态模型识图。与翻译实例独立，需单独添加。',
    builtin: true,
    keyRequired: true,
    canDisable: true,
    canDelete: true,
    multiInstance: true,
    hasModelApi: true,
    defaultModel: 'gpt-4o',
    models: ['gpt-4o', 'gpt-4o-mini'],
    protocolId: 'openai_chat',
    apiBaseUrl: 'https://api.openai.com/v1',
    detailKind: 'vision-llm',
    group: 'vision',
    iconifyId: 'simple-icons:openai',
    docsUrl: 'https://developers.openai.com/api/docs',
    apiKeyUrl: 'https://platform.openai.com/api-keys',
    runtimeSupported: true,
  },
  {
    id: 'claude-vision',
    name: 'Claude',
    description: 'Anthropic Claude 多模态识图。与翻译实例独立，需单独添加。',
    builtin: true,
    keyRequired: true,
    canDisable: true,
    canDelete: true,
    multiInstance: true,
    hasModelApi: true,
    defaultModel: 'claude-haiku-4-5',
    models: ['claude-haiku-4-5', 'claude-3-5-sonnet-latest', 'claude-3-5-haiku-latest'],
    protocolId: 'claude_messages',
    apiBaseUrl: 'https://api.anthropic.com',
    detailKind: 'vision-llm',
    group: 'vision',
    iconifyId: 'simple-icons:anthropic',
    docsUrl: 'https://docs.anthropic.com',
    apiKeyUrl: 'https://console.anthropic.com/settings/keys',
    runtimeSupported: false,
  },
  {
    id: 'gemini-vision',
    name: 'Gemini',
    description: 'Google Gemini 多模态识图。与翻译实例独立，需单独添加。',
    builtin: true,
    keyRequired: true,
    canDisable: true,
    canDelete: true,
    multiInstance: true,
    hasModelApi: true,
    defaultModel: 'gemini-1.5-flash',
    models: ['gemini-1.5-pro', 'gemini-1.5-flash', 'gemini-1.5-flash-8b'],
    protocolId: 'openai_chat',
    // 与翻译 gemini 一致：Google OpenAI 兼容端点
    apiBaseUrl: 'https://generativelanguage.googleapis.com/v1beta/openai',
    detailKind: 'vision-llm',
    group: 'vision',
    iconifyId: 'simple-icons:google',
    docsUrl: 'https://ai.google.dev/docs',
    apiKeyUrl: 'https://aistudio.google.com/apikey',
    runtimeSupported: true,
  },
  {
    id: 'zhipu-vl',
    name: '智谱 AI',
    description: '智谱 GLM-4V 等多模态模型识图。与翻译实例独立，需单独添加。',
    builtin: true,
    keyRequired: true,
    canDisable: true,
    canDelete: true,
    multiInstance: true,
    hasModelApi: true,
    defaultModel: 'glm-4v-flash',
    models: ['glm-4v-flash', 'glm-4v-plus', 'glm-4v'],
    protocolId: 'openai_chat',
    apiBaseUrl: 'https://open.bigmodel.cn/api/paas/v4',
    detailKind: 'vision-llm',
    group: 'vision',
    docsUrl: 'https://open.bigmodel.cn/dev/api',
    apiKeyUrl: 'https://open.bigmodel.cn/usercenter/apikeys',
    runtimeSupported: true,
  },
  {
    id: 'siliconflow-vision',
    name: '硅基流动',
    description: '硅基流动多模态开源模型识图。与翻译实例独立，需单独添加。',
    builtin: true,
    keyRequired: true,
    canDisable: true,
    canDelete: true,
    multiInstance: true,
    hasModelApi: true,
    defaultModel: 'deepseek-ai/DeepSeek-OCR',
    models: [
      'deepseek-ai/DeepSeek-OCR',
      'Qwen/Qwen2.5-VL-7B-Instruct',
      'Qwen/Qwen2.5-VL-32B-Instruct',
      'Qwen/Qwen2.5-VL-72B-Instruct',
    ],
    protocolId: 'openai_chat',
    apiBaseUrl: 'https://api.siliconflow.cn/v1',
    detailKind: 'vision-llm',
    group: 'vision',
    docsUrl: 'https://docs.siliconflow.cn',
    apiKeyUrl: 'https://cloud.siliconflow.cn/account/ak',
    runtimeSupported: true,
  },
  {
    // moonshot 视觉能力以官方文档为准；保留条目供用户接入，以 tokens 为唯一源
    id: 'moonshot-vision',
    name: '月之暗面',
    description: 'Moonshot 多模态识图。与翻译实例独立，需单独添加。',
    builtin: true,
    keyRequired: true,
    canDisable: true,
    canDelete: true,
    multiInstance: true,
    hasModelApi: true,
    defaultModel: 'moonshot-v1-8k-vision-preview',
    models: ['moonshot-v1-8k-vision-preview', 'moonshot-v1-32k-vision-preview'],
    protocolId: 'openai_chat',
    apiBaseUrl: 'https://api.moonshot.cn/v1',
    detailKind: 'vision-llm',
    group: 'vision',
    iconifyId: 'simple-icons:moonshotai',
    docsUrl: 'https://platform.moonshot.cn/docs',
    apiKeyUrl: 'https://platform.moonshot.cn/console/api-keys',
    runtimeSupported: true,
  },
  {
    id: 'openai-compatible-vision',
    name: '自定义 OpenAI 兼容',
    description: '接入任意 OpenAI 兼容多模态端点。与翻译实例独立，需单独添加。',
    builtin: true,
    keyRequired: true,
    canDisable: true,
    canDelete: true,
    multiInstance: true,
    needsEndpoint: true,
    hasModelApi: true,
    defaultModel: '',
    protocolId: 'openai_chat',
    apiBaseUrl: '',
    detailKind: 'vision-llm',
    group: 'vision',
    docsUrl: 'https://developers.openai.com/api/docs',
    runtimeSupported: true,
  },
]

export const ocrServiceById = (id: OcrServiceId): OcrServiceMeta | undefined =>
  BUILTIN_OCR_SERVICES.find((s) => s.id === id)

/**
 * OCR type → 翻译侧 ServiceId，用于列表/添加器图标与翻译渠道一致。
 * Windows 系统 OCR 无对应翻译渠道，返回 undefined（UI 用 ScanText）。
 */
export const ocrTypeToTranslationServiceId = (type: OcrServiceId): ServiceId | undefined => {
  switch (type) {
    case 'openai-vision':
      return 'openai'
    case 'claude-vision':
      return 'claude'
    case 'gemini-vision':
      return 'gemini'
    case 'zhipu-vl':
      return 'zhipu'
    case 'siliconflow-vision':
      return 'siliconflow'
    case 'moonshot-vision':
      return 'moonshot'
    case 'openai-compatible-vision':
      return 'custom'
    default:
      return undefined
  }
}

/** 添加 OCR 服务 picker：仅 vision 组（不含 system / 专用 OCR）。 */
export const OCR_PICKER_SERVICES = BUILTIN_OCR_SERVICES.filter((s) => s.group === 'vision')

/**
 * 模拟从服务商 /models 端点拉取到的模型列表。
 * 真实实现中应在 onPullModels 里调用对应端点并写入 services[id].pulledModels。
 * 这里用于 UI 演示，包含部分"灰度中"或私有模型，让用户能看到下拉内容。
 */
export const MOCK_PULLED_MODELS: Record<ServiceId, string[]> = {
  openai: ['gpt-4o', 'gpt-4o-mini', 'gpt-4o-2024-08-06', 'gpt-4-turbo', 'gpt-3.5-turbo', 'o1-preview', 'o1-mini', 'gpt-4.1'],
  deepseek: ['deepseek-chat', 'deepseek-reasoner', 'deepseek-coder', 'deepseek-v3'],
  claude: [
    'claude-3-5-sonnet-latest',
    'claude-3-5-haiku-latest',
    'claude-3-opus-latest',
    'claude-3-7-sonnet-latest',
  ],
  microsoft: [],
  gemini: ['gemini-2.0-flash-exp', 'gemini-1.5-pro', 'gemini-1.5-flash', 'gemini-1.5-flash-8b'],
  deepl: [],
  google: [],
  baidu: [],
  youdao: [],
  tencent: [],
  volcengine: ['Doubao-pro-32k', 'Doubao-lite-4k', 'Doubao-pro-128k', 'auto-router'],
  iflytek: [],
  zhipu: ['glm-4-plus', 'glm-4-air', 'glm-4-flash', 'glm-zero'],
  moonshot: ['moonshot-v1-8k', 'moonshot-v1-32k', 'moonshot-v1-128k', 'moonshot-v1-auto'],
  siliconflow: [
    'Qwen/Qwen2.5-7B-Instruct',
    'Qwen/Qwen2.5-14B-Instruct',
    'Qwen/Qwen2.5-32B-Instruct',
    'Qwen/Qwen2.5-72B-Instruct',
    'Qwen/Qwen2.5-Coder-32B-Instruct',
    'Qwen/QwQ-32B-Preview',
    'deepseek-ai/DeepSeek-V2.5',
    'deepseek-ai/DeepSeek-R1',
    'THUDM/glm-4-9b-chat',
    'meta-llama/Meta-Llama-3.1-8B-Instruct',
    'meta-llama/Meta-Llama-3.1-70B-Instruct',
  ],
  custom: [],
}
