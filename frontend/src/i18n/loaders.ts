import { createLogger } from '@public/logger.js'
import zhCN from './locales/zh-CN.json'
import enUS from './locales/en-US.json'

export const BUILTIN_LOCALES = ['zh-CN', 'zh-TW', 'en-US', 'ja-JP', 'ko-KR', 'fr-FR', 'de-DE', 'es-ES'] as const
export type BuiltinLocale = typeof BUILTIN_LOCALES[number]
export interface LanguagePackage {
  schemaVersion: number
  locale: string
  name: string
  messages: Record<string, string>
}

const logger = createLogger('i18n')
const fallback = zhCN as LanguagePackage
const staticPackages: Partial<Record<BuiltinLocale, LanguagePackage>> = {
  'zh-CN': fallback,
  'en-US': enUS as LanguagePackage,
}
const dynamicPackages: Partial<Record<BuiltinLocale, () => Promise<{ default: LanguagePackage }>>> = {
  'zh-TW': () => import('./locales/zh-TW.json') as Promise<{ default: LanguagePackage }>,
  'ja-JP': () => import('./locales/ja-JP.json') as Promise<{ default: LanguagePackage }>,
  'ko-KR': () => import('./locales/ko-KR.json') as Promise<{ default: LanguagePackage }>,
  'fr-FR': () => import('./locales/fr-FR.json') as Promise<{ default: LanguagePackage }>,
  'de-DE': () => import('./locales/de-DE.json') as Promise<{ default: LanguagePackage }>,
  'es-ES': () => import('./locales/es-ES.json') as Promise<{ default: LanguagePackage }>,
}

export async function loadBuiltin(locale: string): Promise<LanguagePackage> {
  const builtin = staticPackages[locale as BuiltinLocale]
  if (builtin) return builtin
  const load = dynamicPackages[locale as BuiltinLocale]
  if (!load) return fallback
  try {
    return (await load()).default
  } catch (error) {
    logger.warn('内置语言包加载失败，回退简体中文', {
      locale,
      error: (error instanceof Error ? error.message : String(error)).replace(/[\r\n]+/g, ' ').slice(0, 200),
    })
    throw error
  }
}

export const ZH_CN_PACKAGE = fallback
