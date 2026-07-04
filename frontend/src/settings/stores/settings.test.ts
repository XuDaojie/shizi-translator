import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useSettings } from './settings';

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
