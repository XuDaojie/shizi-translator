import { describe, it, expect } from 'vitest';
import { validateConfig } from './config';
import type { AppConfig } from '@/types/config';

const base: AppConfig = {
  provider: 'openai-compatible',
  targetLang: '中文',
  openaiCompatible: { apiKey: 'sk-x', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', timeoutSeconds: 60 },
  claude: { apiKey: null, baseUrl: 'https://api.anthropic.com', model: 'claude-haiku-4-5', timeoutSeconds: 60, enableThinking: false },
  popupPrecreate: true,
  overlayPrecreate: true,
  collectUsage: true,
};

describe('validateConfig', () => {
  it('mock provider 跳过校验', () => {
    expect(validateConfig({ ...base, provider: 'mock' })).toBeNull();
  });

  it('openai-compatible 有效配置返回 null', () => {
    expect(validateConfig(base)).toBeNull();
  });

  it('baseUrl 非 http(s) 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, baseUrl: 'ftp://x' } }))
      .toBe('Base URL 请输入有效的 http(s) 地址');
  });

  it('baseUrl 非法 URL 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, baseUrl: 'not a url' } }))
      .toBe('Base URL 请输入有效的 http(s) 地址');
  });

  it('model 为空报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, model: '' } }))
      .toBe('Model 不能为空');
  });

  it('timeoutSeconds 小于 1 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 0 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('timeoutSeconds 大于 600 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 601 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('timeoutSeconds 非整数报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 1.5 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('claude provider 校验 claude 段', () => {
    const c = { ...base, provider: 'claude' as const, claude: { ...base.claude, model: '' } };
    expect(validateConfig(c)).toBe('Model 不能为空');
  });

  it('claude provider 有效配置返回 null', () => {
    const c = { ...base, provider: 'claude' as const, claude: { ...base.claude, apiKey: 'sk-ant-x' } };
    expect(validateConfig(c)).toBeNull();
  });
});
