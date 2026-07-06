export function servicePayload(service) {
  return {
    serviceInstanceId: service.id,
    serviceType: service.serviceType,
    serviceName: service.name,
  };
}

export function enabledServicePayloads(config) {
  return (config?.services || [])
    .filter((service) => service.enabled)
    .map(servicePayload);
}

export function syncServiceCards(config, deps) {
  const payloads = enabledServicePayloads(config);
  const enabledIds = new Set(payloads.map((payload) => payload.serviceInstanceId));

  deps.resultCards?.forEach((card, id) => {
    if (deps.allowRemove !== false && !enabledIds.has(id) && card.status !== 'translating') {
      card.el.remove();
      deps.resultCards.delete(id);
    }
  });

  payloads.forEach((payload) => {
    if (deps.allowCreate === false && !deps.resultCards?.has(payload.serviceInstanceId)) return;
    const card = deps.getCard(payload);
    deps.updateCardMeta(card, payload);
    if (!(deps.allowCreate === false && deps.allowRemove === false)) {
      deps.resultsList.appendChild(card.el);
    }
  });

  const sourceLabel = deps.langSource?.querySelector('.lang-label');
  if (sourceLabel) {
    sourceLabel.textContent = !config?.defaultSourceLang || config.defaultSourceLang === 'auto'
      ? '自动检测'
      : config.defaultSourceLang;
  }

  const targetLabel = deps.langTarget?.querySelector('.lang-label');
  if (targetLabel) {
    targetLabel.textContent = config?.targetLang || '中文';
  }
}
