import { describe, expect, it } from 'vitest'
import { getServiceLogoSrc } from './service-logos'
import { getServiceIconifyId } from './tokens'

describe('getServiceLogoSrc', () => {
  it('covers lobe-icons vendored brands', () => {
    for (const id of ['zhipu', 'siliconflow', 'volcengine', 'tencent'] as const) {
      const src = getServiceLogoSrc(id)
      expect(src, id).toBeTruthy()
      expect(src).toMatch(/\.svg/)
      // 有本地 logo 的不应再依赖 simple-icons（避免双源）
      expect(getServiceIconifyId(id)).toBeUndefined()
    }
  })

  it('leaves youdao / iflytek empty (lobe 未收录)', () => {
    expect(getServiceLogoSrc('youdao')).toBeUndefined()
    expect(getServiceLogoSrc('iflytek')).toBeUndefined()
  })
})
