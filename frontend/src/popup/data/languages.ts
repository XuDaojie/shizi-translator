/** 语言代码 ↔ 名称映射。与 frontend/src/settings/tokens.ts 的 LANGUAGES 同源，
 *  新增语言两处同步。弹窗侧多一个 english 字段供搜索 combobox 双列展示。 */
export interface Language {
  value: string
  label: string
  english: string
}

export const LANGUAGES: Language[] = [
  { value: 'auto',  label: '自动检测', english: 'Auto Detect' },
  { value: 'zh-CN', label: '简体中文', english: 'Chinese (Simplified)' },
  { value: 'zh-TW', label: '繁體中文', english: 'Chinese (Traditional)' },
  { value: 'en-US', label: 'English', english: 'English' },
  { value: 'ja-JP', label: '日本語',   english: 'Japanese' },
  { value: 'ko-KR', label: '한국어',   english: 'Korean' },
  { value: 'fr-FR', label: 'Français', english: 'French' },
  { value: 'de-DE', label: 'Deutsch',  english: 'German' },
  { value: 'es-ES', label: 'Español',  english: 'Spanish' },
  { value: 'ru-RU', label: 'Русский',  english: 'Russian' },
]

/** ISO 码 -> 显示名，找不到回退原码。 */
export const langLabel = (code: string): string =>
  LANGUAGES.find((l) => l.value === code)?.label ?? code
