const inputText = document.getElementById('inputText');
const outputText = document.getElementById('outputText');
const sourceBadge = document.getElementById('sourceBadge');
const translateBtn = document.getElementById('translateBtn');
const settingsBtn = document.getElementById('settingsBtn');
const clearBtn = document.getElementById('clearBtn');
const cancelBtn = document.getElementById('cancelBtn');
const retryBtn = document.getElementById('retryBtn');
const usageFooter = document.getElementById('usageFooter');

const tauriApi = window.__TAURI__;
const invoke = tauriApi?.core?.invoke;
const listen = tauriApi?.event?.listen;

let isTranslating = false;
let currentSessionId = null;

function showUsageFooter(usage) {
  if (!usage) {
    hideUsageFooter();
    return;
  }
  usageFooter.textContent = `${usage.inputTokens} → ${usage.outputTokens} tokens`;
  usageFooter.classList.remove('hidden');
}

function hideUsageFooter() {
  usageFooter.classList.add('hidden');
  usageFooter.textContent = '';
}

function resetOutput() {
  outputText.textContent = '翻译结果将显示在这里';
  outputText.style.color = '#999';
  hideUsageFooter();
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
      hideUsageFooter();
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
      showUsageFooter(payload.usage);
      setActionButtons({ translating: false, canRetry: true });
      scrollOutputToBottom();
      break;
    case 'failed':
      if (currentSessionId && !shouldHandleSessionEvent(payload)) return;
      outputText.textContent = payload.message ?? '翻译失败';
      outputText.style.color = '#b42318';
      currentSessionId = null;
      hideSourceBadge();
      hideUsageFooter();
      setActionButtons({ translating: false, canRetry: payload.retryable !== false });
      scrollOutputToBottom();
      break;
    case 'cancelled':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent += '\n[已取消]';
      outputText.style.color = '#999';
      currentSessionId = null;
      hideSourceBadge();
      hideUsageFooter();
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

async function applyPendingSourceText() {
  if (!invoke) return;
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

window.addEventListener('focus', applyPendingSourceText);

translateBtn.addEventListener('click', async () => {
  if (isTranslating) return;

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
  hideUsageFooter();
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

settingsBtn.addEventListener('click', () => {
  if (invoke) {
    invoke('open_settings');
  }
});

clearBtn.addEventListener('click', () => {
  if (isTranslating) return;
  inputText.value = '';
  currentSessionId = null;
  resetOutput();
  hideSourceBadge();
  setActionButtons({ translating: false, canRetry: false });
});

cancelBtn.addEventListener('click', async () => {
  if (!invoke) return;
  try {
    await invoke('cancel_translation');
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
  }
});

retryBtn.addEventListener('click', async () => {
  if (isTranslating) return;
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

applyPendingSourceText();
