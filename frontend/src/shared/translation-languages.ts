export interface TranslationLanguage {
  code: string
  nativeName: string
  promptName: string
  nameKey: `language.${string}`
}

const TARGET_LANGUAGE_DATA = [
  ['zh-CN', '简体中文', 'Chinese (Simplified)'],
  ['zh-TW', '繁體中文', 'Chinese (Traditional)'],
  ['en', 'English', 'English'],
  ['ja', '日本語', 'Japanese'],
  ['ko', '한국어', 'Korean'],
  ['fr', 'Français', 'French'],
  ['de', 'Deutsch', 'German'],
  ['es', 'Español', 'Spanish'],
  ['pt', 'Português', 'Portuguese'],
  ['ru', 'Русский', 'Russian'],
  ['it', 'Italiano', 'Italian'],
  ['nl', 'Nederlands', 'Dutch'],
  ['pl', 'Polski', 'Polish'],
  ['tr', 'Türkçe', 'Turkish'],
  ['ar', 'العربية', 'Arabic'],
  ['th', 'ภาษาไทย', 'Thai'],
  ['vi', 'Tiếng Việt', 'Vietnamese'],
  ['id', 'Bahasa Indonesia', 'Indonesian'],
  ['hi', 'हिन्दी', 'Hindi'],
] as const

export const TARGET_LANGUAGES: TranslationLanguage[] = TARGET_LANGUAGE_DATA.map(
  ([code, nativeName, promptName]) => ({ code, nativeName, promptName, nameKey: `language.${code}` }),
)

export const AUTO_LANGUAGE: TranslationLanguage = {
  code: 'auto',
  nativeName: '自动检测',
  promptName: 'Auto Detect',
  nameKey: 'language.auto',
}

export const SOURCE_LANGUAGES: TranslationLanguage[] = [AUTO_LANGUAGE, ...TARGET_LANGUAGES]
export const TRANSLATION_LANGUAGES = SOURCE_LANGUAGES

export const translationLanguage = (code: string): TranslationLanguage | undefined =>
  TRANSLATION_LANGUAGES.find((item) => item.code === code)
