import { describe, expect, it } from 'vitest'
import { SOURCE_LANGUAGES, TARGET_LANGUAGES, TRANSLATION_LANGUAGES, translationLanguage } from './translation-languages'

const codes = ['zh-CN', 'zh-TW', 'en', 'ja', 'ko', 'fr', 'de', 'es', 'pt', 'ru', 'it', 'nl', 'pl', 'tr', 'ar', 'th', 'vi', 'id', 'hi']
const promptNames = ['Chinese (Simplified)', 'Chinese (Traditional)', 'English', 'Japanese', 'Korean', 'French', 'German', 'Spanish', 'Portuguese', 'Russian', 'Italian', 'Dutch', 'Polish', 'Turkish', 'Arabic', 'Thai', 'Vietnamese', 'Indonesian', 'Hindi']

describe('翻译语言目录', () => {
  it('目标语言恰好包含 19 种规范代码', () => {
    expect(TARGET_LANGUAGES.map((item) => item.code)).toEqual(codes)
  })

  it('源语言仅额外包含一个 auto', () => {
    expect(SOURCE_LANGUAGES.map((item) => item.code)).toEqual(['auto', ...codes])
    expect(TRANSLATION_LANGUAGES.filter((item) => item.code === 'auto')).toHaveLength(1)
    expect(translationLanguage('legacy-code')).toBeUndefined()
  })

  it('每种实际语言都有稳定 prompt 名称和界面消息键', () => {
    expect(TARGET_LANGUAGES.map((item) => item.promptName)).toEqual(promptNames)
    for (const item of TARGET_LANGUAGES) {
      expect(item.promptName).toMatch(/^[A-Za-z ()]+$/)
      expect(item.nameKey).toBe(`language.${item.code}`)
    }
  })
})
