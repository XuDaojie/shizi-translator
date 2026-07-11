import { Plug, WandSparkles } from '@lucide/vue'
import type { Component } from 'vue'
import type { CustomServiceType, ServiceId, ServiceMeta } from './types'

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
 * 厂商官方 logo（Iconify simple-icons 集）有则填，无则统一用 lucide `Plug` 兜底。
 * Logo 候选来源：https://api.iconify.design/?prefix=simple-icons
 *
 * 命名约定：
 * - 优先使用厂商英文品牌对应的 simple-icons 条目
 * - 找不到的（腾讯、讯飞、智谱、字节火山、有道）留空，由 ServiceIcon 自动 fallback
 *   到 lucide `Plug` 图标，**不**使用相似品牌替代、**不**为每个服务单独挑 lucide 图标
 *   避免视觉上看起来像「乱配的」——统一传达「未识别厂商，请走 OpenAI 兼容协议」语义
 */
export const getServiceIconifyId = (id: ServiceId): string | undefined =>
  BUILTIN_SERVICES.find((s) => s.id === id)?.iconifyId

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
  },
  {
    id: 'microsoft',
    name: '微软翻译',
    description: 'Edge 浏览器默认翻译引擎，免 Key，复用浏览器环境信息调用。',
    builtin: true,
    defaultModel: '',
    iconifyId: 'simple-icons:microsoftedge',
    category: 'ml',
    keyRequired: false,
    protocols: [MICROSOFT_EDGE],
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
