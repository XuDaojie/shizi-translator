import { readonly, ref, type Ref } from 'vue'
import {
  invokeGetInterfaceLanguageSnapshot,
  type InterfaceLanguageSnapshot,
} from '@/lib/tauri'
import { loadBuiltin, ZH_CN_PACKAGE, type LanguagePackage } from './loaders'
import zhCN from './locales/zh-CN.json'

export type MessageKey = keyof typeof zhCN.messages
export type MessageParams = Record<string, string | number>
type BuiltinLoader = (locale: string) => Promise<LanguagePackage>
type SnapshotLoader = () => Promise<InterfaceLanguageSnapshot>
const PLACEHOLDER = /\{([A-Za-z][A-Za-z0-9_]*)\}/g

export interface I18nRuntime {
  locale: Readonly<Ref<string>>
  revision: Readonly<Ref<number>>
  userMessages: Readonly<Ref<Record<string, string>>>
  builtinMessages: Readonly<Ref<Record<string, string>>>
  t: (key: MessageKey, params?: MessageParams) => string
  formatDateTime: (value: Date | number | string, options?: Intl.DateTimeFormatOptions) => string
  applySnapshot: (snapshot: InterfaceLanguageSnapshot) => Promise<void>
  reloadCurrentLocale: () => Promise<void>
}

function createRuntime(loader: BuiltinLoader, fetchSnapshot: SnapshotLoader): I18nRuntime {
  const activeLocale = ref('zh-CN')
  const activeRevision = ref(-1)
  const users = ref<Record<string, string>>({})
  const builtins = ref<Record<string, string>>(ZH_CN_PACKAGE.messages)
  let highestRequestedRevision = -1
  let retryableRevision: number | null = null
  let latestRequestId = 0

  const loadSnapshot = async (snapshot: InterfaceLanguageSnapshot): Promise<void> => {
    const isRetry = snapshot.revision === retryableRevision
    if (snapshot.revision < highestRequestedRevision || (snapshot.revision === highestRequestedRevision && !isRetry)) return
    if (snapshot.revision > highestRequestedRevision) highestRequestedRevision = snapshot.revision
    retryableRevision = null
    const requestId = ++latestRequestId
    let pkg: LanguagePackage
    try {
      pkg = await loader(snapshot.locale)
    } catch {
      // 初始 refs 已是 zh-CN；不推进 revision，保留当前状态并允许同 revision 重试。
      if (requestId === latestRequestId) retryableRevision = snapshot.revision
      return
    }
    if (requestId !== latestRequestId || snapshot.revision < highestRequestedRevision) return
    const isUserLocale = snapshot.languages.some(({ locale, builtin }) => locale === snapshot.locale && !builtin)
      || Object.keys(snapshot.userMessages).length > 0
    activeLocale.value = isUserLocale ? snapshot.locale : pkg.locale
    activeRevision.value = snapshot.revision
    users.value = { ...snapshot.userMessages }
    builtins.value = pkg.messages
  }
  const applySnapshot = (snapshot: InterfaceLanguageSnapshot): Promise<void> => loadSnapshot(snapshot)

  const t = (key: MessageKey, params: MessageParams = {}): string => {
    const message = users.value[key] ?? builtins.value[key] ?? ZH_CN_PACKAGE.messages[key] ?? key
    return message.replace(PLACEHOLDER, (placeholder, name: string) =>
      Object.prototype.hasOwnProperty.call(params, name) ? String(params[name]) : placeholder,
    )
  }

  return {
    locale: readonly(activeLocale),
    revision: readonly(activeRevision),
    userMessages: readonly(users),
    builtinMessages: readonly(builtins),
    t,
    formatDateTime: (value, options) => {
      const date = new Date(value)
      return Number.isNaN(date.getTime()) ? '—' : new Intl.DateTimeFormat(activeLocale.value, options).format(date)
    },
    applySnapshot,
    reloadCurrentLocale: async () => applySnapshot(await fetchSnapshot()),
  }
}

const runtime = createRuntime(loadBuiltin, invokeGetInterfaceLanguageSnapshot)
export const locale = runtime.locale
export const revision = runtime.revision
export const userMessages = runtime.userMessages
export const builtinMessages = runtime.builtinMessages
export const t = runtime.t
export const formatDateTime = runtime.formatDateTime
export const applySnapshot = runtime.applySnapshot
export const reloadCurrentLocale = runtime.reloadCurrentLocale
export const initializeI18n = async (snapshot?: InterfaceLanguageSnapshot): Promise<void> =>
  applySnapshot(snapshot ?? await invokeGetInterfaceLanguageSnapshot())
export const createI18nForTest = (
  loader: BuiltinLoader = loadBuiltin,
  fetchSnapshot: SnapshotLoader = invokeGetInterfaceLanguageSnapshot,
): I18nRuntime => createRuntime(loader, fetchSnapshot)
