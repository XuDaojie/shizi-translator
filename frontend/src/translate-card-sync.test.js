import { describe, expect, it } from 'vitest';
import { syncServiceCards } from '../public/translate-card-sync.js';

function makeDeps(existingIds = []) {
  const children = [];
  const resultCards = new Map();
  const updates = [];
  const created = [];

  function makeCard(id, overrides = {}) {
    created.push(id);
    const el = {
      id,
      removed: false,
      remove() {
        this.removed = true;
        const index = children.indexOf(this);
        if (index >= 0) children.splice(index, 1);
      },
    };
    const card = { el, ...overrides };
    resultCards.set(id, card);
    children.push(el);
    return card;
  }

  existingIds.forEach(makeCard);

  return {
    children,
    resultCards,
    updates,
    created,
    makeCard,
    getCard(payload) {
      return resultCards.get(payload.serviceInstanceId) || makeCard(payload.serviceInstanceId);
    },
    updateCardMeta(card, payload) {
      card.meta = payload;
      updates.push([card.el.id, payload.serviceName, payload.serviceType]);
    },
    resultsList: {
      appendChild(el) {
        const index = children.indexOf(el);
        if (index >= 0) children.splice(index, 1);
        children.push(el);
      },
    },
  };
}

describe('syncServiceCards', () => {
  it('按启用服务新增、删除并排序卡片', () => {
    const deps = makeDeps(['old', 'a']);

    syncServiceCards({
      services: [
        { id: 'b', serviceType: 'claude', name: 'Claude', enabled: true },
        { id: 'disabled', serviceType: 'mock', name: 'Mock', enabled: false },
        { id: 'a', serviceType: 'deepseek', name: 'DeepSeek', enabled: true },
      ],
    }, deps);

    expect([...deps.resultCards.keys()]).toEqual(['a', 'b']);
    expect(deps.children.map((el) => el.id)).toEqual(['b', 'a']);
  });

  it('服务名称或类型变化时更新已有卡片元信息', () => {
    const deps = makeDeps(['svc']);
    const sourceLabel = { textContent: '' };
    const targetLabel = { textContent: '' };

    syncServiceCards({
      targetLang: '',
      services: [
        { id: 'svc', serviceType: 'openai', name: '新名称', enabled: true },
      ],
    }, {
      ...deps,
      langSource: { querySelector: () => sourceLabel },
      langTarget: { querySelector: () => targetLabel },
    });

    expect(deps.updates).toEqual([['svc', '新名称', 'openai']]);
    expect(deps.resultCards.get('svc').meta).toEqual({
      serviceInstanceId: 'svc',
      serviceType: 'openai',
      serviceName: '新名称',
    });
    expect(sourceLabel.textContent).toBe('自动检测');
    expect(targetLabel.textContent).toBe('中文');
  });

  it('禁用正在翻译的服务时保留卡片和现有文本', () => {
    const deps = makeDeps();
    const translating = deps.makeCard('svc', { status: 'translating', text: { textContent: '已有输出' } });

    syncServiceCards({
      services: [
        { id: 'other', serviceType: 'mock', name: 'Mock', enabled: true },
      ],
    }, deps);

    expect(deps.resultCards.get('svc')).toBe(translating);
    expect(translating.el.removed).toBe(false);
    expect(translating.text.textContent).toBe('已有输出');
  });

  it('allowCreate 为 false 且 allowRemove 为 false 时只同步已有卡片', () => {
    const deps = makeDeps(['existing']);
    const disabledFinished = deps.makeCard('disabled', { status: 'finished', text: { textContent: '完成结果' } });
    deps.created.length = 0;

    syncServiceCards({
      services: [
        { id: 'missing', serviceType: 'claude', name: 'Claude', enabled: true },
        { id: 'existing', serviceType: 'openai', name: '已有服务新名', enabled: true },
        { id: 'disabled', serviceType: 'mock', name: '已禁用', enabled: false },
      ],
    }, {
      ...deps,
      allowCreate: false,
      allowRemove: false,
    });

    expect(deps.created).toEqual([]);
    expect(deps.resultCards.has('missing')).toBe(false);
    expect(deps.resultCards.get('disabled')).toBe(disabledFinished);
    expect(disabledFinished.el.removed).toBe(false);
    expect(disabledFinished.text.textContent).toBe('完成结果');
    expect(deps.updates).toEqual([['existing', '已有服务新名', 'openai']]);
    expect(deps.children.map((el) => el.id)).toEqual(['existing', 'disabled']);
  });
});
