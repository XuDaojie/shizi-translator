import { describe, expect, it } from 'vitest'

import type { InterfaceLanguageSnapshot } from '@/lib/tauri'
import { createI18nForTest } from './index'
import { BUILTIN_LOCALES, loadBuiltin, type LanguagePackage } from './loaders'
import zhCN from './locales/zh-CN.json'
import zhTW from './locales/zh-TW.json'
import enUS from './locales/en-US.json'
import jaJP from './locales/ja-JP.json'
import koKR from './locales/ko-KR.json'
import frFR from './locales/fr-FR.json'
import deDE from './locales/de-DE.json'
import esES from './locales/es-ES.json'

const packages = [zhCN, zhTW, enUS, jaJP, koKR, frFR, deDE, esES]

const snapshot = (
  locale: string,
  revision: number,
  userMessages: Record<string, string> = {},
): InterfaceLanguageSnapshot => ({
  configuredLocale: locale,
  locale,
  revision,
  languages: [],
  userMessages,
  errors: [],
})

describe('内置语言包', () => {
  it('8 份字典元数据有效且消息键与简体中文完全一致', () => {
    expect(BUILTIN_LOCALES).toEqual(['zh-CN', 'zh-TW', 'en-US', 'ja-JP', 'ko-KR', 'fr-FR', 'de-DE', 'es-ES'])
    const zhKeys = Object.keys(zhCN.messages).sort()
    for (const [index, pkg] of packages.entries()) {
      expect(pkg.schemaVersion).toBe(1)
      expect(pkg.locale).toBe(BUILTIN_LOCALES[index])
      expect(pkg.name).toBeTruthy()
      expect(Object.keys(pkg.messages).sort()).toEqual(zhKeys)
      expect(Object.values(pkg.messages).every((message) => typeof message === 'string')).toBe(true)
    }
  })

  it('loadBuiltin 加载内置语言并对加载失败明确回退简体中文', async () => {
    for (const locale of BUILTIN_LOCALES) expect((await loadBuiltin(locale)).locale).toBe(locale)
    expect((await loadBuiltin('not-a-locale')).locale).toBe('zh-CN')
  })
})

describe('i18n 运行时', () => {
  it('按用户字典、同语言内置、简体中文、键名依次回退并插值', async () => {
    const builtin: LanguagePackage = {
      schemaVersion: 1,
      locale: 'en-US',
      name: 'English',
      messages: { 'common.save': 'Save', greeting: 'Hello, {name} / {missing}' },
    }
    const i18n = createI18nForTest(async (locale) => locale === 'en-US' ? builtin : zhCN)
    await i18n.applySnapshot(snapshot('en-US', 1, { 'common.save': 'Store' }))

    expect(i18n.t('common.save')).toBe('Store')
    expect(i18n.t('greeting', { name: 'Ada' })).toBe('Hello, Ada / {missing}')
    expect(i18n.t('test.zhOnly')).toBe(zhCN.messages['test.zhOnly'])
    expect(i18n.t('missing.key')).toBe('missing.key')
  })

  it('同 locale 的新 revision 重新应用用户字典，相同 revision 幂等', async () => {
    let loads = 0
    const i18n = createI18nForTest(async () => { loads++; return enUS })
    await i18n.applySnapshot(snapshot('en-US', 1, { 'common.save': 'First' }))
    await i18n.applySnapshot(snapshot('en-US', 1, { 'common.save': 'Ignored' }))
    expect(i18n.t('common.save')).toBe('First')
    expect(loads).toBe(1)

    await i18n.applySnapshot(snapshot('en-US', 2, { 'common.save': 'Second' }))
    expect(i18n.t('common.save')).toBe('Second')
    expect(loads).toBe(2)
  })

  it('主动重载获取并应用后端最新快照', async () => {
    let fetches = 0
    const i18n = createI18nForTest(
      async () => enUS,
      async () => { fetches++; return snapshot('en-US', 4, { 'common.save': 'Reloaded' }) },
    )
    await i18n.applySnapshot(snapshot('en-US', 3))
    await i18n.reloadCurrentLocale()
    expect(fetches).toBe(1)
    expect(i18n.revision.value).toBe(4)
    expect(i18n.t('common.save')).toBe('Reloaded')
  })

  it('主动重载忽略后端旧 revision', async () => {
    const i18n = createI18nForTest(
      async () => enUS,
      async () => snapshot('en-US', 2, { 'common.save': 'Old' }),
    )
    await i18n.applySnapshot(snapshot('en-US', 3, { 'common.save': 'Current' }))
    await i18n.reloadCurrentLocale()
    expect(i18n.revision.value).toBe(3)
    expect(i18n.t('common.save')).toBe('Current')
  })

  it('主动重载获取失败时保留当前有效状态并向调用者抛出', async () => {
    const failure = new Error('snapshot unavailable')
    const i18n = createI18nForTest(
      async () => enUS,
      async () => { throw failure },
    )
    await i18n.applySnapshot(snapshot('en-US', 3, { 'common.save': 'Current' }))
    await expect(i18n.reloadCurrentLocale()).rejects.toBe(failure)
    expect(i18n.revision.value).toBe(3)
    expect(i18n.t('common.save')).toBe('Current')
  })

  it('并发加载时旧 revision 不覆盖新 revision', async () => {
    let resolveOld!: (pkg: LanguagePackage) => void
    const oldLoad = new Promise<LanguagePackage>((resolve) => { resolveOld = resolve })
    const i18n = createI18nForTest((locale) => locale === 'fr-FR' ? oldLoad : Promise.resolve(enUS))

    const pending = i18n.applySnapshot(snapshot('fr-FR', 1, { value: 'old' }))
    await i18n.applySnapshot(snapshot('en-US', 2, { value: 'new' }))
    resolveOld(frFR)
    await pending

    expect(i18n.locale.value).toBe('en-US')
    expect(i18n.revision.value).toBe(2)
    expect(i18n.t('value')).toBe('new')
  })

  it('先发起的高 revision 不会被后发起的低 revision 淘汰', async () => {
    let resolveNew!: (pkg: LanguagePackage) => void
    const newLoad = new Promise<LanguagePackage>((resolve) => { resolveNew = resolve })
    const i18n = createI18nForTest((locale) => locale === 'fr-FR' ? newLoad : Promise.resolve(enUS))

    const pending = i18n.applySnapshot(snapshot('fr-FR', 4, { value: 'new' }))
    await i18n.applySnapshot(snapshot('en-US', 3, { value: 'old' }))
    resolveNew(frFR)
    await pending

    expect(i18n.locale.value).toBe('fr-FR')
    expect(i18n.revision.value).toBe(4)
    expect(i18n.t('value')).toBe('new')
  })

  it('加载器抛错时保持可用并回退简体中文', async () => {
    const i18n = createI18nForTest(async () => { throw new Error('chunk missing') })
    await expect(i18n.applySnapshot(snapshot('fr-FR', 1))).resolves.toBeUndefined()
    expect(i18n.locale.value).toBe('zh-CN')
    expect(i18n.t('common.save')).toBe(zhCN.messages['common.save'])
  })

  it('formatDateTime 使用当前 locale', async () => {
    const i18n = createI18nForTest(async () => enUS)
    await i18n.applySnapshot(snapshot('en-US', 1))
    const date = new Date(2024, 0, 2, 3, 4)
    expect(i18n.formatDateTime(date, { year: 'numeric' })).toBe(
      new Intl.DateTimeFormat('en-US', { year: 'numeric' }).format(date),
    )
  })
})
