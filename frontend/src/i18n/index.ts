import { readonly, ref, type Ref } from 'vue'
import {
  invokeGetInterfaceLanguageSnapshot,
  type InterfaceLanguageSnapshot,
} from '@/lib/tauri'
import { loadBuiltin, ZH_CN_PACKAGE, type LanguagePackage } from './loaders'

type Params = Record<string, string | number>
type BuiltinLoader = (locale: string) => Promise<LanguagePackage>
const PLACEHOLDER = /\{([A-Za-z][A-Za-z0-9_]*)\}/g

export interface I18nRuntime {
  locale: Readonly<Ref<string>>
  revision: Readonly<Ref<number>>
  userMessages: Readonly<Ref<Record<string, string>>>
  builtinMessages: Readonly<Ref<Record<string, string>>>
  t: (key: string, params?: Params) => string
  formatDateTime: (value: Date | number | string, options?: Intl.DateTimeFormatOptions) => string
  applySnapshot: (snapshot: InterfaceLanguageSnapshot) => Promise<void>
  reloadCurrentLocale: () => Promise<void>
}

function createRuntime(loader: BuiltinLoader): I18nRuntime {
  const activeLocale = ref('zh-CN')
  const activeRevision = ref(-1)
  const users = ref<Record<string, string>>({})
  const builtins = ref<Record<string, string>>(ZH_CN_PACKAGE.messages)
  let currentSnapshot: InterfaceLanguageSnapshot | undefined
  let highestRequestedRevision = -1
  let requestId = 0

  const loadSnapshot = async (snapshot: InterfaceLanguageSnapshot, force = false): Promise<void> => {
    if (force ? snapshot.revision < highestRequestedRevision : snapshot.revision <= highestRequestedRevision) return
    highestRequestedRevision = snapshot.revision
    const id = ++requestId
    let pkg: LanguagePackage
    try {
      pkg = await loader(snapshot.locale)
    } catch {
      pkg = ZH_CN_PACKAGE
    }
    if (snapshot.revision < highestRequestedRevision || id !== requestId) return
    activeLocale.value = pkg.locale
    activeRevision.value = snapshot.revision
    users.value = { ...snapshot.userMessages }
    builtins.value = pkg.messages
    currentSnapshot = snapshot
  }
  const applySnapshot = (snapshot: InterfaceLanguageSnapshot): Promise<void> => loadSnapshot(snapshot)

  const t = (key: string, params: Params = {}): string => {
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
    formatDateTime: (value, options) => new Intl.DateTimeFormat(activeLocale.value, options).format(new Date(value)),
    applySnapshot,
    reloadCurrentLocale: async () => {
      if (!currentSnapshot) return
      await loadSnapshot(currentSnapshot, true)
    },
  }
}

const runtime = createRuntime(loadBuiltin)
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
export const createI18nForTest = (loader: BuiltinLoader = loadBuiltin): I18nRuntime => createRuntime(loader)
