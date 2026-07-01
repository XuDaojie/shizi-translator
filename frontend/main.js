const inputText = document.getElementById('inputText');
const outputText = document.getElementById('outputText');
const sourceBadge = document.getElementById('sourceBadge');
const translateBtn = document.getElementById('translateBtn');
const settingsBtn = document.getElementById('settingsBtn');
const clearBtn = document.getElementById('clearBtn');
const cancelBtn = document.getElementById('cancelBtn');
const retryBtn = document.getElementById('retryBtn');
const settingsPanel = document.getElementById('settingsPanel');
const targetLangInput = document.getElementById('targetLangInput');
const apiKeyInput = document.getElementById('apiKeyInput');
const baseUrlInput = document.getElementById('baseUrlInput');
const modelInput = document.getElementById('modelInput');
const timeoutInput = document.getElementById('timeoutInput');
const saveConfigBtn = document.getElementById('saveConfigBtn');
const configStatus = document.getElementById('configStatus');
const providerSelect = document.getElementById('providerSelect');
const openaiSettings = document.getElementById('openaiSettings');
const claudeSettings = document.getElementById('claudeSettings');
const claudeApiKeyInput = document.getElementById('claudeApiKeyInput');
const claudeBaseUrlInput = document.getElementById('claudeBaseUrlInput');
const claudeModelInput = document.getElementById('claudeModelInput');
const claudeTimeoutInput = document.getElementById('claudeTimeoutInput');
const claudeEnableThinkingInput = document.getElementById('claudeEnableThinkingInput');

const tauriApi = window.__TAURI__;
const invoke = tauriApi?.core?.invoke;
const listen = tauriApi?.event?.listen;

let isTranslating = false;
let currentSessionId = null;

function resetOutput() {
  outputText.textContent = '翻译结果将显示在这里';
  outputText.style.color = '#999';
}

function setSourceBadge(sourceType) {
  switch (sourceType) {
    case 'selectedText':
      sourceBadge.textContent = '来自划词';
      sourceBadge.classList.remove('hidden');
      break;
    case 'ocrText':
      sourceBadge.textContent = '来自 OCR';
      sourceBadge.classList.remove('hidden');
      break;
    default:
      // manualText 或未知值：隐藏（防御）
      sourceBadge.classList.add('hidden');
      sourceBadge.textContent = '';
      break;
  }
}

function hideSourceBadge() {
  sourceBadge.classList.add('hidden');
  sourceBadge.textContent = '';
}

function setActionButtons({ translating, canRetry }) {
  isTranslating = translating;
  translateBtn.disabled = translating;
  clearBtn.disabled = translating;
  translateBtn.textContent = translating ? '翻译中...' : '翻译';
  cancelBtn.hidden = !translating;
  retryBtn.hidden = !canRetry;
  retryBtn.disabled = translating;
}

function setConfigStatus(message, isError = false) {
  configStatus.textContent = message;
  configStatus.style.color = isError ? '#b42318' : '#666';
}

function scrollOutputToBottom() {
  outputText.scrollTop = outputText.scrollHeight;
}

function getSessionId(payload) {
  const sessionId = payload?.sessionId;
  if (typeof sessionId === 'string') {
    return sessionId;
  }
  if (sessionId && typeof sessionId === 'object') {
    return sessionId[0] ?? sessionId['0'] ?? null;
  }
  return null;
}

function toggleProviderSettings() {
  const provider = providerSelect.value;
  openaiSettings.classList.toggle('hidden', provider !== 'openai-compatible');
  claudeSettings.classList.toggle('hidden', provider !== 'claude');
}

function fillConfigForm(config) {
  targetLangInput.value = config.targetLang ?? '中文';
  providerSelect.value = config.provider ?? 'openai-compatible';
  // OpenAI Compatible 字段
  apiKeyInput.value = config.openaiCompatible?.apiKey ?? '';
  baseUrlInput.value = config.openaiCompatible?.baseUrl ?? 'https://api.openai.com/v1';
  modelInput.value = config.openaiCompatible?.model ?? 'gpt-4o-mini';
  timeoutInput.value = String(config.openaiCompatible?.timeoutSeconds ?? 60);
  // Claude 字段
  claudeApiKeyInput.value = config.claude?.apiKey ?? '';
  claudeBaseUrlInput.value = config.claude?.baseUrl ?? 'https://api.anthropic.com';
  claudeModelInput.value = config.claude?.model ?? 'claude-haiku-4-5';
  claudeTimeoutInput.value = String(config.claude?.timeoutSeconds ?? 60);
  claudeEnableThinkingInput.checked = config.claude?.enableThinking ?? false;
  toggleProviderSettings();
}

function readConfigForm() {
  return {
    provider: providerSelect.value,
    targetLang: targetLangInput.value.trim() || '中文',
    openaiCompatible: {
      apiKey: apiKeyInput.value.trim() || null,
      baseUrl: baseUrlInput.value.trim(),
      model: modelInput.value.trim(),
      timeoutSeconds: Number(timeoutInput.value),
    },
    claude: {
      apiKey: claudeApiKeyInput.value.trim() || null,
      baseUrl: claudeBaseUrlInput.value.trim(),
      model: claudeModelInput.value.trim(),
      timeoutSeconds: Number(claudeTimeoutInput.value),
      enableThinking: claudeEnableThinkingInput.checked,
    },
  };
}

function validateConfig(config) {
  if (config.provider === 'mock') return null;
  const sections = config.provider === 'claude' ? [config.claude] : [config.openaiCompatible];
  for (const section of sections) {
    let url;
    try {
      url = new URL(section.baseUrl);
    } catch {
      return 'Base URL 请输入有效的 http(s) 地址';
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return 'Base URL 请输入有效的 http(s) 地址';
    }
    if (!section.model) {
      return 'Model 不能为空';
    }
    if (!Number.isInteger(section.timeoutSeconds)
        || section.timeoutSeconds < 1
        || section.timeoutSeconds > 600) {
      return 'Timeout 秒请输入 1-600 的整数';
    }
  }
  return null;
}

async function loadAppConfig() {
  if (!invoke) {
    setConfigStatus('Tauri API 未就绪，无法读取配置', true);
    return;
  }

  try {
    const config = await invoke('get_app_config');
    fillConfigForm(config);
    setConfigStatus('');
  } catch (error) {
    setConfigStatus(String(error), true);
  }
}

async function saveAppConfig() {
  if (!invoke) {
    setConfigStatus('Tauri API 未就绪，无法保存配置', true);
    return;
  }

  const configToSave = readConfigForm();
  const validationError = validateConfig(configToSave);
  if (validationError) {
    setConfigStatus(validationError, true);
    return;
  }

  saveConfigBtn.disabled = true;
  saveConfigBtn.textContent = '保存中...';
  setConfigStatus('保存中...');

  try {
    const config = await invoke('save_app_config', { config: configToSave });
    fillConfigForm(config);
    setConfigStatus('配置已保存，下一次翻译生效');
  } catch (error) {
    setConfigStatus(String(error), true);
  } finally {
    saveConfigBtn.disabled = false;
    saveConfigBtn.textContent = '保存配置';
  }
}

async function applyPendingSourceText() {
  if (!invoke) {
    return;
  }

  try {
    const sourceText = await invoke('take_pending_source_text');
    if (sourceText) {
      inputText.value = sourceText;
    }
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
  }
}

function shouldHandleSessionEvent(payload) {
  const sessionId = getSessionId(payload);
  return !currentSessionId || !sessionId || sessionId === currentSessionId;
}

function renderTranslationEvent(payload) {
  switch (payload.type) {
    case 'started':
      currentSessionId = getSessionId(payload);
      inputText.value = payload.sourceText ?? inputText.value;
      outputText.textContent = '';
      outputText.style.color = '#333';
      setSourceBadge(payload.sourceType);
      setActionButtons({ translating: true, canRetry: false });
      break;
    case 'delta':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent += payload.text ?? '';
      outputText.style.color = '#333';
      scrollOutputToBottom();
      break;
    case 'finished':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent = payload.fullText ?? outputText.textContent;
      outputText.style.color = '#333';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      scrollOutputToBottom();
      break;
    case 'failed':
      if (currentSessionId && !shouldHandleSessionEvent(payload)) return;
      outputText.textContent = payload.message ?? '翻译失败';
      outputText.style.color = '#b42318';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      scrollOutputToBottom();
      break;
    case 'cancelled':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent += '\n[已取消]';
      outputText.style.color = '#999';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      break;
    default:
      break;
  }
}

if (listen) {
  listen('translation:event', (event) => {
    renderTranslationEvent(event.payload);
  });
}

window.addEventListener('focus', applyPendingSourceText);

function syncSettingsButtonText() {
  settingsBtn.textContent = settingsPanel.classList.contains('hidden') ? '设置' : '收起设置';
}

settingsBtn.addEventListener('click', () => {
  settingsPanel.classList.toggle('hidden');
  syncSettingsButtonText();
});

providerSelect.addEventListener('change', toggleProviderSettings);

saveConfigBtn.addEventListener('click', saveAppConfig);

translateBtn.addEventListener('click', async () => {
  if (isTranslating) {
    return;
  }

  const text = inputText.value.trim();
  if (!text) {
    outputText.textContent = '请输入要翻译的文本';
    outputText.style.color = '#999';
    return;
  }

  if (!invoke) {
    outputText.textContent = 'Tauri API 未就绪，请在桌面应用中运行';
    outputText.style.color = '#b42318';
    return;
  }

  outputText.textContent = '翻译中...';
  outputText.style.color = '#999';
  setActionButtons({ translating: true, canRetry: false });

  try {
    await invoke('start_translation', { text });
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
    currentSessionId = null;
    hideSourceBadge();
    setActionButtons({ translating: false, canRetry: true });
  }
});

clearBtn.addEventListener('click', () => {
  if (isTranslating) {
    return;
  }
  inputText.value = '';
  currentSessionId = null;
  resetOutput();
  hideSourceBadge();
  setActionButtons({ translating: false, canRetry: false });
});

cancelBtn.addEventListener('click', async () => {
  if (!invoke) {
    return;
  }
  try {
    await invoke('cancel_translation');
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
  }
});

retryBtn.addEventListener('click', async () => {
  if (isTranslating) {
    return;
  }
  if (!invoke) {
    outputText.textContent = 'Tauri API 未就绪，请在桌面应用中运行';
    outputText.style.color = '#b42318';
    return;
  }
  outputText.textContent = '翻译中...';
  outputText.style.color = '#999';
  setActionButtons({ translating: true, canRetry: false });
  try {
    await invoke('retry_translation');
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
    currentSessionId = null;
    hideSourceBadge();
    setActionButtons({ translating: false, canRetry: true });
  }
});

inputText.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    translateBtn.click();
  }
});

syncSettingsButtonText();
loadAppConfig();
applyPendingSourceText();
