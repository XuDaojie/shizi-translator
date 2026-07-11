import { describe, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import { useTranslationEvents, type CardState, type TranslationEventPayload } from './useTranslationEvents'

/** 构造最小可用 opts，记录回调调用。 */
function makeHarness() {
  const cards = reactive<Map<string, CardState>>(new Map())
  const state = { isTranslating: false, batchId: null as string | null }
  const calls = {
    started: [] as Array<{ payload: TranslationEventPayload; isNewBatch: boolean }>,
    batchStatus: 0,
    detected: [] as Array<string | null>,
    config: [] as Array<unknown>,
  }
  const logger = { info: vi.fn(), warn: vi.fn() }
  const listen = vi.fn(async (_evt: string, handler: (e: { payload: unknown }) => void) => {
    ;(listen as unknown as { _handler: typeof handler })._handler = handler
    return () => {}
  })
  vi.stubGlobal('window', {
    __TAURI__: { event: { listen } },
  })
  const { dispatch } = useTranslationEvents({
    cards,
    getIsTranslating: () => state.isTranslating,
    setIsTranslating: (v) => { state.isTranslating = v },
    getCurrentBatchId: () => state.batchId,
    setCurrentBatchId: (id) => { state.batchId = id },
    onStarted: (payload, isNewBatch) => { calls.started.push({ payload, isNewBatch }); state.isTranslating = true },
    onBatchStatusChange: () => { calls.batchStatus++ },
    onDetectedLang: (lang) => { calls.detected.push(lang) },
    onConfigChanged: (cfg) => { calls.config.push(cfg) },
    logger,
  })
  return { cards, state, calls, dispatch, listen }
}

describe('useTranslationEvents.dispatch', () => {
  it('started 新 batch 创建卡片并标记 translating', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'OpenAI', serviceType: 'openai', modelName: 'gpt-4o-mini', sourceText: 'hi', sourceType: 'selectedText' })
    expect(h.cards.get('svc-a')).toBeDefined()
    expect(h.cards.get('svc-a')!.status).toBe('translating')
    expect(h.cards.get('svc-a')!.serviceName).toBe('OpenAI')
    expect(h.cards.get('svc-a')!.modelName).toBe('gpt-4o-mini')
    expect(h.calls.started[0].isNewBatch).toBe(true)
  })

  it('delta 追加 text 到对应卡片', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: 'Hel' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: 'lo' })
    expect(h.cards.get('svc-a')!.text).toBe('Hello')
  })

  it('finished 全量替换 text 并写入 usage/detectedSourceLang', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '部分' })
    h.dispatch({
      type: 'finished', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a',
      fullText: '完整译文', usage: { inputTokens: 10, outputTokens: 20 }, detectedSourceLang: 'en-US',
    })
    const card = h.cards.get('svc-a')!
    expect(card.text).toBe('完整译文')
    expect(card.status).toBe('finished')
    expect(card.usage).toEqual({ inputTokens: 10, outputTokens: 20 })
    expect(card.detectedSourceLang).toBe('en-US')
    expect(card.showActions).toBe(true)
    expect(h.calls.detected).toContain('en-US')
  })

  it('failed 设置错误文本与 failed 状态', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'failed', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', message: '网络错误' })
    const card = h.cards.get('svc-a')!
    expect(card.status).toBe('failed')
    expect(card.text).toBe('网络错误')
    expect(card.showActions).toBe(false)
  })

  it('cancelled 追加 [已取消] 标记', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '部分' })
    h.dispatch({ type: 'cancelled', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a' })
    const card = h.cards.get('svc-a')!
    expect(card.status).toBe('cancelled')
    expect(card.text).toContain('[已取消]')
  })

  it('batchId 切换时重置所有已有卡片（新 batch）', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '旧文本' })
    // 新 batch
    h.dispatch({ type: 'started', sessionId: 'batch-2:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    expect(h.cards.get('svc-a')!.text).toBe('')
    expect(h.cards.get('svc-a')!.status).toBe('translating')
  })

  it('跨 batch 的陈旧 delta 被丢弃', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'started', sessionId: 'batch-2:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '陈旧' })
    expect(h.cards.get('svc-a')!.text).toBe('')
  })

  it('started 同 batch 新服务实例新建卡片，不重置已有', () => {
    const h = makeHarness()
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', serviceName: 'A', serviceType: 'openai' })
    h.dispatch({ type: 'delta', sessionId: 'batch-1:svc-a', serviceInstanceId: 'svc-a', text: '保留' })
    h.dispatch({ type: 'started', sessionId: 'batch-1:svc-b', serviceInstanceId: 'svc-b', serviceName: 'B', serviceType: 'claude' })
    expect(h.cards.get('svc-a')!.text).toBe('保留')
    expect(h.cards.get('svc-b')!.status).toBe('translating')
  })
})

describe('useTranslationEvents.collapsed 状态机', () => {
  it('started 后 collapsed 仍为 true（不因 started 展开）', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(true)
    expect(h.cards.get('svc-a')!.status).toBe('translating')
  })

  it('首条非空 delta 后该卡 collapsed=false', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: 'Hel',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    expect(h.cards.get('svc-a')!.text).toBe('Hel')
  })

  it('空 delta 不展开', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: '',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(true)
  })

  it('failed 无正文也展开', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'failed',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      message: '网络错误',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    expect(h.cards.get('svc-a')!.status).toBe('failed')
  })

  it('仅 finished（无中间 delta）展开', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'finished',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      fullText: '完整译文',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    expect(h.cards.get('svc-a')!.text).toBe('完整译文')
  })

  it('多服务：A 出字只展开 A，B 仍收缩', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-b',
      serviceInstanceId: 'svc-b',
      serviceName: 'B',
      serviceType: 'claude',
    })
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: '仅 A',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    expect(h.cards.get('svc-b')!.collapsed).toBe(true)
  })

  it('新 batch 先收回再各自等首包', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: '旧',
    })
    expect(h.cards.get('svc-a')!.collapsed).toBe(false)
    h.dispatch({
      type: 'started',
      sessionId: 'batch-2:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    expect(h.cards.get('svc-a')!.text).toBe('')
    expect(h.cards.get('svc-a')!.collapsed).toBe(true)
  })

  it('用户 override 后首 delta 不自动展开', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    const card = h.cards.get('svc-a')!
    card.collapseUserOverride = true
    card.collapsed = true
    h.dispatch({
      type: 'delta',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      text: 'Hello',
    })
    expect(card.collapsed).toBe(true)
    expect(card.text).toBe('Hello')
  })

  it('新 batch 清除 override 并恢复默认收缩', () => {
    const h = makeHarness()
    h.dispatch({
      type: 'started',
      sessionId: 'batch-1:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    const card = h.cards.get('svc-a')!
    card.collapseUserOverride = true
    card.collapsed = true
    h.dispatch({
      type: 'started',
      sessionId: 'batch-2:svc-a',
      serviceInstanceId: 'svc-a',
      serviceName: 'A',
      serviceType: 'openai',
    })
    expect(card.collapseUserOverride).toBe(false)
    expect(card.collapsed).toBe(true)
  })
})
