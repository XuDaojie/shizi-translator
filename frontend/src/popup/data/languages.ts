import { TRANSLATION_LANGUAGES } from '@/shared/translation-languages'

export interface Language {
  value: string
  label: string
  english: string
}

export const LANGUAGES: Language[] = TRANSLATION_LANGUAGES.map((item) => ({
  value: item.code,
  label: item.nativeName,
  english: item.promptName,
}))

/** ISO 码 -> 显示名，找不到回退原码。 */
export const langLabel = (code: string): string =>
  LANGUAGES.find((l) => l.value === code)?.label ?? code
