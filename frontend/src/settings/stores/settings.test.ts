import { nextTick } from 'vue';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { applyBackendLogLevel, applyShortcutConflicts, mergeBackendIntoServices, useSettings } from './settings';
import { DEFAULT_PROMPTS } from '../tokens';
import type { ServiceInstanceConfig } from '@/types/config';
import { invokeGetAppConfig, invokeGetInterfaceLanguageSnapshot, invokeGetShortcutConflicts, invokeRefreshInterfaceLanguages, invokeSaveAppConfig, isTauriReady } from '@/lib/tauri';
import type { AppSettings, ServiceInstance } from '../types';
import type { InterfaceLanguageSnapshot } from '@/lib/tauri';

// Mock tauri module so tests don't need window.__TAURI__
vi.mock('@/lib/tauri', () => ({
  invokeGetAppConfig: vi.fn(),
  invokeSaveAppConfig: vi.fn(),
  invokeGetShortcutConflicts: vi.fn().mockResolvedValue([]),
  invokeGetInterfaceLanguageSnapshot: vi.fn(),
  invokeRefreshInterfaceLanguages: vi.fn(),
  invokeOpenLanguagePackDirectory: vi.fn(),
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
  vi.resetAllMocks();
  vi.mocked(invokeGetShortcutConflicts).mockResolvedValue([]);
  vi.mocked(isTauriReady).mockReturnValue(false);
  fakeLocalStorage.clear();
  useSettings().reset();
});

const languageSnapshot = (over: Record<string, unknown> = {}) => ({
  configuredLocale: 'auto',
  locale: 'zh-CN',
  revision: 1,
  languages: [
    { locale: 'zh-CN', name: '简体中文', builtin: true },
    { locale: 'it-IT', name: 'Italiano', builtin: false },
  ],
  userMessages: {},
  errors: [],
  ...over,
});

const deferred = <T>() => {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

describe('interface languages', () => {
  it('切换界面语言立即保存且不被 debounce 重复保存', async () => {
    vi.useFakeTimers()
    vi.mocked(isTauriReady).mockReturnValue(true)
    const settings = useSettings()
    vi.mocked(invokeSaveAppConfig).mockImplementation(async (config) => config)

    const saving = settings.setInterfaceLanguage('it-IT')

    expect(settings.state.general.language).toBe('it-IT')
    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1)
    expect(vi.mocked(invokeSaveAppConfig).mock.calls[0][0].interfaceLanguage).toBe('it-IT')
    await saving
    await vi.advanceTimersByTimeAsync(300)
    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1)
  })

  it('界面语言立即保存失败时保留选择和未保存状态', async () => {
    Object.assign(window, { setTimeout })
    vi.mocked(isTauriReady).mockReturnValue(true)
    vi.mocked(invokeSaveAppConfig).mockRejectedValue(new Error('save failed'))
    const settings = useSettings()

    await settings.setInterfaceLanguage('de-DE')

    expect(settings.state.general.language).toBe('de-DE')
    expect(settings.dirty.value).toBe(true)
    expect(settings.saveStatus.value).toBe('error')
  })

  it('并发刷新只应用最后发起请求的结果', async () => {
    const first = deferred<InterfaceLanguageSnapshot>()
    const second = deferred<InterfaceLanguageSnapshot>()
    vi.mocked(invokeRefreshInterfaceLanguages)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const settings = useSettings()

    const requestA = settings.refreshInterfaceLanguages()
    const requestB = settings.refreshInterfaceLanguages()
    second.resolve(languageSnapshot({
      languages: [{ locale: 'de-DE', name: 'Deutsch', builtin: true }],
      errors: [{ file: 'new.json', message: 'new' }],
    }))
    await requestB
    first.resolve(languageSnapshot({
      languages: [{ locale: 'fr-FR', name: 'Français', builtin: true }],
      errors: [{ file: 'old.json', message: 'old' }],
    }))
    await requestA

    expect(settings.interfaceLanguages.value.map(({ locale }) => locale)).toEqual(['de-DE'])
    expect(settings.interfaceLanguageErrors.value).toEqual([{ file: 'new.json', message: 'new' }])
  })

  it('旧刷新先结束时不提前关闭最新请求的 loading', async () => {
    const first = deferred<InterfaceLanguageSnapshot>()
    const second = deferred<InterfaceLanguageSnapshot>()
    vi.mocked(invokeRefreshInterfaceLanguages)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const settings = useSettings()

    const requestA = settings.refreshInterfaceLanguages()
    const requestB = settings.refreshInterfaceLanguages()
    first.resolve(languageSnapshot())
    await requestA
    expect(settings.interfaceLanguagesRefreshing.value).toBe(true)

    second.resolve(languageSnapshot())
    await requestB
    expect(settings.interfaceLanguagesRefreshing.value).toBe(false)
  })

  it('刷新后当前语言已不存在时回写后端解析语言并走自动保存', async () => {
    vi.useFakeTimers();
    vi.mocked(isTauriReady).mockReturnValue(true);
    const settings = useSettings();
    settings.state.general.language = 'it-IT';
    vi.mocked(invokeRefreshInterfaceLanguages).mockResolvedValue(languageSnapshot({
      configuredLocale: 'zh-CN',
      languages: [{ locale: 'zh-CN', name: '简体中文', builtin: true }],
    }));

    await settings.refreshInterfaceLanguages();
    expect(settings.state.general.language).toBe('zh-CN');
    await vi.advanceTimersByTimeAsync(350);
    expect(invokeSaveAppConfig).toHaveBeenCalled();
  });

  it('刷新后保留仍然有效的当前语言', async () => {
    const settings = useSettings();
    settings.state.general.language = 'it-IT';
    vi.mocked(invokeRefreshInterfaceLanguages).mockResolvedValue(languageSnapshot());

    await settings.refreshInterfaceLanguages();

    expect(settings.state.general.language).toBe('it-IT');
  });

  it('按后端顺序保留语言 metadata 和语言包错误详情', async () => {
    const settings = useSettings();
    vi.mocked(invokeRefreshInterfaceLanguages).mockResolvedValue(languageSnapshot({
      languages: [
        { locale: 'auto', name: '忽略', builtin: false },
        { locale: 'de-DE', name: 'Deutsch', builtin: true },
        { locale: 'x-team', name: 'Team', builtin: false },
      ],
      errors: [{ file: 'broken.json', message: 'JSON 无效' }],
    }));

    await settings.refreshInterfaceLanguages();

    expect(settings.interfaceLanguages.value).toEqual([
      { locale: 'de-DE', name: 'Deutsch', builtin: true },
      { locale: 'x-team', name: 'Team', builtin: false },
    ]);
    expect(settings.interfaceLanguageErrors.value).toEqual([
      { file: 'broken.json', message: 'JSON 无效' },
    ]);
  });

  it('刷新失败时保留现有运行时状态并向调用者抛错', async () => {
    const settings = useSettings();
    vi.mocked(invokeRefreshInterfaceLanguages)
      .mockResolvedValueOnce(languageSnapshot())
      .mockRejectedValueOnce(new Error('refresh failed'));
    await settings.refreshInterfaceLanguages();

    await expect(settings.refreshInterfaceLanguages()).rejects.toThrow('refresh failed');
    expect(settings.interfaceLanguages.value).toHaveLength(2);
    expect(settings.interfaceLanguagesRefreshing.value).toBe(false);
  });
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
  it('并发同步复用同一 flight，完成后允许再次同步', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true)
    const backend = deferred<Awaited<ReturnType<typeof invokeGetAppConfig>>>()
    const snapshot = deferred<InterfaceLanguageSnapshot>()
    vi.mocked(invokeGetAppConfig).mockReturnValueOnce(backend.promise)
    vi.mocked(invokeGetInterfaceLanguageSnapshot).mockReturnValueOnce(snapshot.promise)
    const settings = useSettings()

    const first = settings.syncFromBackend()
    const second = settings.syncFromBackend()
    expect(invokeGetAppConfig).toHaveBeenCalledTimes(1)

    backend.resolve({
      interfaceLanguage: 'auto', targetLang: 'zh-CN', defaultSourceLang: 'auto',
      autoCopy: true, restoreClipboard: true, historyLimit: 500, services: [],
      popupPrecreate: true, overlayPrecreate: true, collectUsage: true, logLevel: 'info', shortcuts: {},
    })
    await Promise.resolve()
    expect(invokeGetInterfaceLanguageSnapshot).toHaveBeenCalledTimes(1)
    snapshot.resolve(languageSnapshot())
    await Promise.all([first, second])

    vi.mocked(invokeGetAppConfig).mockResolvedValueOnce({
      interfaceLanguage: 'auto', targetLang: 'zh-CN', defaultSourceLang: 'auto',
      autoCopy: true, restoreClipboard: true, historyLimit: 500, services: [],
      popupPrecreate: true, overlayPrecreate: true, collectUsage: true, logLevel: 'info', shortcuts: {},
    })
    vi.mocked(invokeGetInterfaceLanguageSnapshot).mockResolvedValueOnce(languageSnapshot())
    await settings.syncFromBackend()
    expect(invokeGetAppConfig).toHaveBeenCalledTimes(2)
  })

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
      interfaceLanguage: 'auto',
      targetLang: '中文',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      historyLimit: 500,
      services: [],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      logLevel: 'info',
      shortcuts: {},
    });

    const settings = useSettings();
    const expectedIds = settings.state.services.map((s) => s.id);
    await settings.syncFromBackend();

    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1);
    const saved = vi.mocked(invokeSaveAppConfig).mock.calls[0][0];
    expect(saved.services.map((s) => s.id)).toEqual(expectedIds);
  });

  it('空 services 的语言回退只保存一次，后续用户修改按新状态自动保存', async () => {
    vi.useFakeTimers()
    vi.mocked(isTauriReady).mockReturnValue(true)
    const settings = useSettings()
    settings.state.general.language = 'it-IT'
    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      interfaceLanguage: 'it-IT', targetLang: 'zh-CN', defaultSourceLang: 'auto',
      autoCopy: true, restoreClipboard: true, historyLimit: 500, services: [],
      popupPrecreate: true, overlayPrecreate: true, collectUsage: true, logLevel: 'info', shortcuts: {},
    })
    vi.mocked(invokeGetInterfaceLanguageSnapshot).mockResolvedValueOnce(languageSnapshot({
      configuredLocale: 'zh-CN', locale: 'zh-CN',
      languages: [{ locale: 'zh-CN', name: '简体中文', builtin: true }],
    }))

    await settings.syncFromBackend()
    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1)
    expect(vi.mocked(invokeSaveAppConfig).mock.calls[0][0].interfaceLanguage).toBe('zh-CN')
    await vi.advanceTimersByTimeAsync(350)
    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1)

    settings.state.translation.autoCopy = false
    await nextTick()
    await vi.advanceTimersByTimeAsync(350)
    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(2)
    expect(vi.mocked(invokeSaveAppConfig).mock.calls.map(([config]) => config.autoCopy)).toEqual([true, false])
  })

  it('空 services 首次推送失败时保留 dirty/error 且不启动自动重试', async () => {
    vi.useFakeTimers()
    Object.assign(window, { setTimeout })
    const settings = useSettings()
    vi.mocked(isTauriReady).mockReturnValue(false)
    await settings.save()
    vi.clearAllMocks()
    vi.mocked(isTauriReady).mockReturnValue(true)
    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      interfaceLanguage: 'auto', targetLang: 'zh-CN', defaultSourceLang: 'auto',
      autoCopy: true, restoreClipboard: true, historyLimit: 500, services: [],
      popupPrecreate: true, overlayPrecreate: true, collectUsage: true, logLevel: 'info', shortcuts: {},
    })
    vi.mocked(invokeSaveAppConfig).mockRejectedValueOnce(new Error('write failed'))

    await settings.syncFromBackend()

    expect(settings.dirty.value).toBe(true)
    expect(settings.saveStatus.value).toBe('error')
    await vi.advanceTimersByTimeAsync(350)
    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1)
  })

  it('首次推送等待期间的用户修改在旧推送失败后恢复自动保存', async () => {
    vi.useFakeTimers()
    vi.mocked(isTauriReady).mockReturnValue(true)
    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      interfaceLanguage: 'auto', targetLang: 'zh-CN', defaultSourceLang: 'auto',
      autoCopy: true, restoreClipboard: true, historyLimit: 500, services: [],
      popupPrecreate: true, overlayPrecreate: true, collectUsage: true, logLevel: 'info', shortcuts: {},
    })
    vi.mocked(invokeGetInterfaceLanguageSnapshot).mockResolvedValue(languageSnapshot())
    const pendingSave = deferred<Awaited<ReturnType<typeof invokeSaveAppConfig>>>()
    vi.mocked(invokeSaveAppConfig)
      .mockReturnValueOnce(pendingSave.promise)
      .mockImplementation(async (config) => config)
    const settings = useSettings()

    const sync = settings.syncFromBackend()
    await vi.waitFor(() => expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1))
    settings.state.translation.autoCopy = false
    await nextTick()
    pendingSave.reject(new Error('old push failed'))
    await sync
    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(1)

    await vi.advanceTimersByTimeAsync(350)
    expect(invokeSaveAppConfig).toHaveBeenCalledTimes(2)
    expect(vi.mocked(invokeSaveAppConfig).mock.calls[1][0].autoCopy).toBe(false)
  })

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
      interfaceLanguage: 'fr-FR',
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
      historyLimit: 123,
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      logLevel: 'info',
      shortcuts: {},
    });

    await settings.syncFromBackend();

    expect(settings.state.general.language).toBe('fr-FR');
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
    expect(settings.state.translation.historyLimit).toBe(123);
    expect(invokeSaveAppConfig).not.toHaveBeenCalled();
  });

  it('后端配置的界面语言已不存在时按语言快照回退', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    const settings = useSettings();
    const localId = settings.state.services[0].id;
    vi.mocked(invokeGetInterfaceLanguageSnapshot).mockResolvedValue(languageSnapshot({
      configuredLocale: 'zh-CN',
      locale: 'zh-CN',
      languages: [{ locale: 'zh-CN', name: '简体中文', builtin: true }],
    }));
    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      interfaceLanguage: 'it-IT', targetLang: 'zh-CN', services: [makeBackend({ id: localId })],
      defaultSourceLang: 'auto', autoCopy: true, restoreClipboard: true, historyLimit: 500,
      popupPrecreate: true, overlayPrecreate: true, collectUsage: true, logLevel: 'info', shortcuts: {},
    });

    await settings.syncFromBackend();

    expect(settings.state.general.language).toBe('zh-CN');
  });
  it('后端非空时把 shortcuts 合并回本地绑定，只覆盖 keys', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    const settings = useSettings();
    const localId = settings.state.services[0].id;
    const before = settings.state.shortcut.bindings.find((b) => b.id === 'translate-selection')!;

    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      interfaceLanguage: 'auto',
      targetLang: '中文',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      historyLimit: 500,
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
      logLevel: 'info',
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
      interfaceLanguage: 'auto',
      targetLang: '中文',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      historyLimit: 500,
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
      logLevel: 'info',
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
        interfaceLanguage: 'auto',
        targetLang: '中文',
        defaultSourceLang: 'auto',
        autoCopy: true,
        restoreClipboard: true,
        historyLimit: 500,
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
        logLevel: 'info',
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
      interfaceLanguage: 'auto',
      targetLang: '中文',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      historyLimit: 500,
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
      logLevel: 'info',
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
        interfaceLanguage: 'auto',
        targetLang: '中文',
        defaultSourceLang: 'auto',
        autoCopy: true,
        restoreClipboard: true,
        historyLimit: 500,
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
        logLevel: 'info',
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

describe('applyBackendLogLevel', () => {
  it('后端有效值覆盖前端', () => {
    expect(applyBackendLogLevel('info', 'debug')).toBe('debug')
    expect(applyBackendLogLevel('debug', 'error')).toBe('error')
  })

  it('后端 undefined 保留前端', () => {
    expect(applyBackendLogLevel('info', undefined)).toBe('info')
  })

  it('后端非法值保留前端', () => {
    expect(applyBackendLogLevel('warn', 'trace')).toBe('warn')
    expect(applyBackendLogLevel('warn', '')).toBe('warn')
  })
})

describe('defaultTargetLang', () => {
  it('defaultTargetLang 默认为 zh-CN', () => {
    vi.mocked(isTauriReady).mockReturnValue(false);
    const settings = useSettings();
    expect(settings.state.translation.defaultTargetLang).toBe('zh-CN');
  });

  it('syncFromBackend 回读 targetLang 到 defaultTargetLang', async () => {
    vi.mocked(isTauriReady).mockReturnValue(true);
    vi.mocked(invokeGetAppConfig).mockResolvedValue({
      interfaceLanguage: 'auto',
      targetLang: 'en-US',
      defaultSourceLang: 'auto',
      autoCopy: true,
      restoreClipboard: true,
      historyLimit: 500,
      services: [{ id: 'svc-1', serviceType: 'llm', name: 'A', enabled: true, protocol: 'openai_chat', apiKey: 'k', endpoint: 'e', model: 'm', timeoutSeconds: 60, systemPrompt: '', translationPrompt: '', reflectionPrompt: '', reflectionEnabled: false, chainOfThought: 'off' }],
      popupPrecreate: true,
      overlayPrecreate: true,
      collectUsage: true,
      logLevel: 'info',
      shortcuts: {},
    });
    const settings = useSettings();
    await settings.syncFromBackend();
    expect(settings.state.translation.defaultTargetLang).toBe('en-US');
  });
});
