import { nextTick } from 'vue';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { applyShortcutConflicts, mergeBackendIntoServices, useSettings } from './settings';
import { DEFAULT_PROMPTS } from '../tokens';
import type { ServiceInstanceConfig } from '@/types/config';
import { invokeGetAppConfig, invokeGetShortcutConflicts, invokeSaveAppConfig, isTauriReady } from '@/lib/tauri';
import type { AppSettings, ServiceInstance } from '../types';

// Mock tauri module so tests don't need window.__TAURI__
vi.mock('@/lib/tauri', () => ({
  invokeGetAppConfig: vi.fn(),
  invokeSaveAppConfig: vi.fn(),
  invokeGetShortcutConflicts: vi.fn().mockResolvedValue([]),
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
  vi.useRealTimers();
  fakeLocalStorage.clear();
  useSettings().reset();
  vi.clearAllMocks();
});

describe('settings defaults', () => {
  it('首次启动 OCR 历史为空，不再展示样本数据', () => {
    const { state } = useSettings();

    expect(state.ocrHistory).toEqual([]);
  });

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
  it('默认快捷键使用 Alt+D 和 Alt+E', () => {
    const { state } = useSettings();

    expect(Object.fromEntries(state.shortcut.bindings.map((b) => [b.id, b.keys]))).toMatchObject({
      'translate-selection': 'Alt+D',
      'translate-screenshot': 'Alt+E',
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
  systemPrompt: '',
  translationPrompt: '',
  reflectionPrompt: '',
  reflectionEnabled: false,
  chainOfThought: 'off',
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
        systemPrompt: '后端系统',
        translationPrompt: '后端翻译',
        reflectionPrompt: '后端反思',
        reflectionEnabled: true,
        chainOfThought: 'short',
      }),
    ]

    const result = mergeBackendIntoServices(local, backend)

    expect(result).toHaveLength(1)
    expect(result[0].apiKey).toBe('new-key')
    expect(result[0].enabled).toBe(true)
    expect(result[0].endpoint).toBe('https://new')
    expect(result[0].model).toBe('new-model')
    expect(result[0].systemPrompt).toBe('后端系统')
    expect(result[0].translationPrompt).toBe('后端翻译')
    expect(result[0].reflectionPrompt).toBe('后端反思')
    expect(result[0].reflectionEnabled).toBe(true)
    expect(result[0].chainOfThought).toBe('short')
    expect(result[0].note).toBe('我的备注')
  })

  it('后端多出的实例补进前端，独有字段用默认值', () => {
    const local: ServiceInstance[] = []
    const backend = [
      makeBackend({
        id: 'extra',
        name: '新服务',
        serviceType: 'claude',
        protocol: 'claude_messages',
        systemPrompt: undefined,
        translationPrompt: undefined,
        reflectionPrompt: undefined,
      }),
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

  it('后端空提示词覆盖本地旧自定义提示词', () => {
    const local = [
      makeLocal({
        id: 'a',
        systemPrompt: '旧系统提示词',
        translationPrompt: '旧翻译提示词',
        reflectionPrompt: '旧反思提示词',
      }),
    ]
    const backend = [
      makeBackend({
        id: 'a',
        systemPrompt: '',
        translationPrompt: '',
        reflectionPrompt: '',
      }),
    ]

    const result = mergeBackendIntoServices(local, backend)

    expect(result[0].systemPrompt).toBe('')
    expect(result[0].translationPrompt).toBe('')
    expect(result[0].reflectionPrompt).toBe('')
  })

  it('后端新增服务的空提示词保持为空', () => {
    const result = mergeBackendIntoServices([], [
      makeBackend({
        id: 'extra',
        systemPrompt: '',
        translationPrompt: '',
        reflectionPrompt: '',
      }),
    ])

    expect(result[0].systemPrompt).toBe('')
    expect(result[0].translationPrompt).toBe('')
    expect(result[0].reflectionPrompt).toBe('')
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

describe('syncFromBackend', () => {
  it('Tauri 未就绪时静默降级，不调用任何 invoke', async () => {
    vi.mocked(isTauriReady).mockReturnValue(false);
    const settings = useSettings();
    await settings.syncFromBackend();
    expect(invokeGetAppConfig).not.toHaveBeenCalled();
    expect(invokeSaveAppConfig).not.toHaveBeenCalled();
  });

  it('后端 services 为空时推前端配置覆盖后端', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      targetLang: '中文',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      services: [],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      shortcuts: {},
    });

    const settings = useSettings();
    const expectedIds = settings.state.services.map((s) => s.id);
    await settings.syncFromBackend();

    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1);
    const saved = vi.mocked(invokeSaveAppConfig).mock.calls[0][0];
    expect(saved.services.map((s) => s.id)).toEqual(expectedIds);
  });

  it('invokeGetAppConfig 抛错时静默降级', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    vi.mocked(invokeGetAppConfig).mockRejectedValue(new Error('boom'));
    const settings = useSettings();
    await settings.syncFromBackend();
    expect(invokeSaveAppConfig).not.toHaveBeenCalled();
  });

  it('后端非空时按 id 合并到 state，不推覆盖', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    const settings = useSettings();
    const localId = settings.state.services[0].id;

    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      targetLang: '中文',
      services: [
        {
          id: localId,
          serviceType: 'deepseek',
          name: 'DeepSeek',
          enabled: true,
          protocol: 'openai_chat',
          apiKey: 'backend-key',
          endpoint: 'https://api.deepseek.com',
          model: 'deepseek-chat',
          timeoutSeconds: 60,
          systemPrompt: '系统提示',
          translationPrompt: '翻译提示',
          reflectionPrompt: '反思提示',
          reflectionEnabled: true,
          chainOfThought: 'medium',
        },
        {
          id: 'extra',
          serviceType: 'claude',
          name: 'Claude',
          enabled: false,
          protocol: 'claude_messages',
          apiKey: null,
          endpoint: 'https://api.anthropic.com',
          model: 'claude-haiku-4-5',
          timeoutSeconds: 60,
          systemPrompt: '',
          translationPrompt: '',
          reflectionPrompt: '',
          reflectionEnabled: false,
          chainOfThought: 'off',
        },
      ],
      defaultSourceLang: 'en',
      autoCopy: false,
      restoreClipboard: false,
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      shortcuts: {},
    });

    await settings.syncFromBackend();

    expect(settings.state.services.map((s) => s.id)).toEqual([localId, 'extra']);
    expect(settings.state.services[0].apiKey).toBe('backend-key');
    expect(settings.state.services[0].enabled).toBe(true);
    expect(settings.state.services[0].systemPrompt).toBe('系统提示');
    expect(settings.state.services[0].translationPrompt).toBe('翻译提示');
    expect(settings.state.services[0].reflectionPrompt).toBe('反思提示');
    expect(settings.state.services[0].reflectionEnabled).toBe(true);
    expect(settings.state.services[0].chainOfThought).toBe('medium');
    expect(settings.state.translation.defaultSourceLang).toBe('en');
    expect(settings.state.translation.autoCopy).toBe(false);
    expect(settings.state.translation.restoreClipboard).toBe(false);
    expect(invokeSaveAppConfig).not.toHaveBeenCalled();
  });
  it('后端非空时把 shortcuts 合并回本地绑定，只覆盖 keys', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    const settings = useSettings();
    const localId = settings.state.services[0].id;
    const before = settings.state.shortcut.bindings.find((b) => b.id === 'translate-selection')!;

    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      targetLang: '中文',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      services: [
        makeBackend({
          id: localId,
          serviceType: 'deepseek',
          name: 'DeepSeek',
          enabled: false,
          protocol: 'openai_chat',
          apiKey: null,
          endpoint: 'https://api.deepseek.com',
          model: 'deepseek-chat',
          timeoutSeconds: 60,
        }),
      ],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      shortcuts: {
        'translate-selection': 'Ctrl+Alt+D',
        'translate-screenshot': '',
      },
    });

    await settings.syncFromBackend();

    const byId = Object.fromEntries(settings.state.shortcut.bindings.map((b) => [b.id, b]));
    expect(byId['translate-selection'].keys).toBe('Ctrl+Alt+D');
    expect(byId['translate-selection'].label).toBe(before.label);
    expect(byId['translate-screenshot'].keys).toBe('');
    expect(invokeSaveAppConfig).not.toHaveBeenCalled();
  });

  it('同步后把后端快捷键冲突写入对应 binding.error', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    const settings = useSettings();
    const localId = settings.state.services[0].id;

    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      targetLang: '中文',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      services: [
        makeBackend({
          id: localId,
          serviceType: 'deepseek',
          name: 'DeepSeek',
          enabled: false,
          protocol: 'openai_chat',
          apiKey: null,
          endpoint: 'https://api.deepseek.com',
          model: 'deepseek-chat',
          timeoutSeconds: 60,
        }),
      ],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      shortcuts: { 'translate-selection': 'Alt+D' },
    });
    vi.mocked(invokeGetShortcutConflicts).mockResolvedValue([
      { id: 'translate-selection', message: '已被其他应用占用' },
    ]);

    await settings.syncFromBackend();

    const sel = settings.state.shortcut.bindings.find((b) => b.id === 'translate-selection')!;
    expect(sel.error).toBe('已被其他应用占用');
    const ocr = settings.state.shortcut.bindings.find((b) => b.id === 'translate-screenshot')!;
    expect(ocr.error).toBeUndefined();
  });

  it('设置变更后自动保存到后端', async () => {
    vi.useFakeTimers();
    try {
      vi.mocked(isTauriReady).mockReturnValue(true);
      const settings = useSettings();
      const localId = settings.state.services[0].id;

      vi.mocked(invokeGetAppConfig).mockResolvedValue({
        targetLang: '中文',
        defaultSourceLang: 'auto',
        autoCopy: true,
        restoreClipboard: true,
        services: [
          makeBackend({
            id: localId,
            serviceType: 'deepseek',
            name: 'DeepSeek',
            enabled: false,
            protocol: 'openai_chat',
            apiKey: null,
            endpoint: 'https://api.deepseek.com',
            model: 'deepseek-chat',
            timeoutSeconds: 60,
          }),
        ],
        popupPrecreate: true,
        overlayPrecreate: true,
        collectUsage: true,
        shortcuts: {},
      });
      await settings.syncFromBackend();
      vi.mocked(invokeSaveAppConfig).mockClear();

      settings.state.translation.autoCopy = false;
      await nextTick();
      expect(invokeSaveAppConfig).not.toHaveBeenCalled();

      await vi.advanceTimersByTimeAsync(350);

      expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1);
      expect(vi.mocked(invokeSaveAppConfig).mock.calls[0][0].autoCopy).toBe(false);
      expect(settings.dirty.value).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it('后端同步完成后默认保存状态为本机偏好语义的 idle', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    const settings = useSettings();
    const localId = settings.state.services[0].id;

    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      targetLang: '中文',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      services: [
        makeBackend({
          id: localId,
          serviceType: 'deepseek',
          name: 'DeepSeek',
          enabled: false,
          protocol: 'openai_chat',
          apiKey: null,
          endpoint: 'https://api.deepseek.com',
          model: 'deepseek-chat',
          timeoutSeconds: 60,
        }),
      ],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      shortcuts: {},
    });

    await settings.syncFromBackend();

    expect(settings.saveStatus.value).toBe('idle');
  });

  it('校验状态变化不触发自动保存', async () => {
    vi.useFakeTimers();
    try {
      vi.mocked(isTauriReady).mockReturnValue(true);
      const settings = useSettings();
      const localId = settings.state.services[0].id;

      vi.mocked(invokeGetAppConfig).mockResolvedValue({
        targetLang: '中文',
        defaultSourceLang: 'auto',
        autoCopy: true,
        restoreClipboard: true,
        services: [
          makeBackend({
            id: localId,
            serviceType: 'deepseek',
            name: 'DeepSeek',
            enabled: false,
            protocol: 'openai_chat',
            apiKey: null,
            endpoint: 'https://api.deepseek.com',
            model: 'deepseek-chat',
            timeoutSeconds: 60,
          }),
        ],
        popupPrecreate: true,
        overlayPrecreate: true,
        collectUsage: true,
        shortcuts: {},
      });
      await settings.syncFromBackend();
      vi.mocked(invokeSaveAppConfig).mockClear();

      settings.state.services[0].keyStatus = 'validating';
      await nextTick();
      await vi.advanceTimersByTimeAsync(350);

      expect(invokeSaveAppConfig).not.toHaveBeenCalled();
      expect(settings.dirty.value).toBe(false);
      expect(settings.saveStatus.value).toBe('idle');
    } finally {
      vi.useRealTimers();
    }
  });

});

describe('applyShortcutConflicts', () => {
  it('按 id 写入冲突 message，未列出的清空 error', () => {
    const bindings = [
      { id: 'translate-selection', label: '划词翻译', description: '', keys: 'Alt+D', error: undefined },
      { id: 'translate-clipboard', label: '剪贴板翻译', description: '', keys: 'Ctrl+Shift+C', error: '旧错误' },
    ] as AppSettings['shortcut']['bindings'];

    const result = applyShortcutConflicts(bindings, [
      { id: 'translate-clipboard', message: '已被其他应用占用' },
    ]);

    expect(result[0].error).toBeUndefined();
    expect(result[1].error).toBe('已被其他应用占用');
  });

  it('空冲突列表清空所有 error', () => {
    const bindings = [
      { id: 'translate-selection', label: '划词翻译', description: '', keys: 'Alt+D', error: '旧错误' },
    ] as AppSettings['shortcut']['bindings'];

    const result = applyShortcutConflicts(bindings, []);

    expect(result[0].error).toBeUndefined();
  });
});
