import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mergeBackendIntoServices, useSettings } from './settings';
import { DEFAULT_PROMPTS } from '../tokens';
import type { ServiceInstanceConfig } from '@/types/config';
import type { ServiceInstance } from '../types';

// Mock tauri module so tests don't need window.__TAURI__
vi.mock('@/lib/tauri', () => ({
  invokeSaveAppConfig: vi.fn(),
  isTauriReady: vi.fn(() => false),
}));

// Minimal browser-like globals (Storage is not available in Node)
const fakeLocalStorage = (() => {
  const store: Record<string, string> = {};
  return {
    getItem: (k: string) => store[k] ?? null,
    setItem: (k: string, v: string) => { store[k] = v; },
    removeItem: (k: string) => { delete store[k]; },
    clear: () => { for (const k of Object.keys(store)) delete store[k]; },
    get length() { return Object.keys(store).length; },
    key: (i: number) => Object.keys(store)[i] ?? null,
  };
})();
vi.stubGlobal('window', { localStorage: fakeLocalStorage, __TAURI__: undefined });
vi.stubGlobal('crypto', { randomUUID: () => 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx' });

beforeEach(() => {
  fakeLocalStorage.clear();
});

describe('settings defaults', () => {
  it('首启只展示 DeepSeek 和智谱 AI，且默认关闭', () => {
    const { state } = useSettings();

    expect(state.services.map((s) => s.type)).toEqual(['deepseek', 'zhipu']);
    expect(state.services.map((s) => s.enabled)).toEqual([false, false]);
    expect(state.services.map((s) => s.protocol)).toEqual(['openai_chat', 'openai_chat']);
    expect(state.services[0].endpoint).toBe('https://api.deepseek.com');
    expect(state.services[1].endpoint).toBe('https://open.bigmodel.cn/api/paas/v4');
    expect(state.services[1].model).toBe('glm-4-flash');
  });
});


const makeLocal = (over: Partial<ServiceInstance>): ServiceInstance => ({
  id: 'local-1',
  type: 'deepseek',
  name: 'DeepSeek',
  enabled: false,
  protocol: 'openai_chat',
  apiKey: '',
  model: 'deepseek-chat',
  endpoint: 'https://api.deepseek.com',
  note: '',
  pulledModels: [],
  keyStatus: 'idle',
  chainOfThought: 'off',
  systemPrompt: '',
  translationPrompt: '',
  reflectionPrompt: '',
  reflectionEnabled: false,
  ...over,
})

const makeBackend = (over: Partial<ServiceInstanceConfig>): ServiceInstanceConfig => ({
  id: 'b-1',
  serviceType: 'deepseek',
  name: 'DeepSeek',
  enabled: true,
  protocol: 'openai_chat',
  apiKey: 'sk-x',
  endpoint: 'https://api.deepseek.com',
  model: 'deepseek-chat',
  timeoutSeconds: 60,
  ...over,
})

describe('mergeBackendIntoServices', () => {
  it('后端核心字段覆盖前端同 id 实例，前端独有字段保留', () => {
    const local = [
      makeLocal({
        id: 'a',
        apiKey: 'old-key',
        enabled: false,
        endpoint: 'https://old',
        model: 'old-model',
        systemPrompt: '我的提示词',
        chainOfThought: 'long',
        note: '我的备注',
      }),
    ]
    const backend = [
      makeBackend({
        id: 'a',
        apiKey: 'new-key',
        enabled: true,
        endpoint: 'https://new',
        model: 'new-model',
        protocol: 'openai_chat',
      }),
    ]

    const result = mergeBackendIntoServices(local, backend)

    expect(result).toHaveLength(1)
    expect(result[0].apiKey).toBe('new-key')
    expect(result[0].enabled).toBe(true)
    expect(result[0].endpoint).toBe('https://new')
    expect(result[0].model).toBe('new-model')
    expect(result[0].systemPrompt).toBe('我的提示词')
    expect(result[0].chainOfThought).toBe('long')
    expect(result[0].note).toBe('我的备注')
  })

  it('后端多出的实例补进前端，独有字段用默认值', () => {
    const local: ServiceInstance[] = []
    const backend = [
      makeBackend({ id: 'extra', name: '新服务', serviceType: 'claude', protocol: 'claude_messages' }),
    ]

    const result = mergeBackendIntoServices(local, backend)

    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('extra')
    expect(result[0].systemPrompt).toBe(DEFAULT_PROMPTS.system)
    expect(result[0].translationPrompt).toBe(DEFAULT_PROMPTS.translation)
    expect(result[0].keyStatus).toBe('idle')
    expect(result[0].chainOfThought).toBe('off')
    expect(result[0].pulledModels).toEqual([])
  })

  it('前端多出的实例被删除', () => {
    const local = [
      makeLocal({ id: 'local-only' }),
      makeLocal({ id: 'shared' }),
    ]
    const backend = [makeBackend({ id: 'shared' })]

    const result = mergeBackendIntoServices(local, backend)

    expect(result.map((s) => s.id)).toEqual(['shared'])
  })

  it('结果顺序按后端 services 顺序', () => {
    const local = [makeLocal({ id: 'a' }), makeLocal({ id: 'b' })]
    const backend = [makeBackend({ id: 'b' }), makeBackend({ id: 'a' })]

    const result = mergeBackendIntoServices(local, backend)

    expect(result.map((s) => s.id)).toEqual(['b', 'a'])
  })
})
