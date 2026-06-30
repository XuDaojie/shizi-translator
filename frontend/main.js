const inputText = document.getElementById('inputText');
const outputText = document.getElementById('outputText');
const translateBtn = document.getElementById('translateBtn');
const settingsBtn = document.getElementById('settingsBtn');
const clearBtn = document.getElementById('clearBtn');
const settingsPanel = document.getElementById('settingsPanel');
const targetLangInput = document.getElementById('targetLangInput');
const apiKeyInput = document.getElementById('apiKeyInput');
const baseUrlInput = document.getElementById('baseUrlInput');
const modelInput = document.getElementById('modelInput');
const timeoutInput = document.getElementById('timeoutInput');
const saveConfigBtn = document.getElementById('saveConfigBtn');
const configStatus = document.getElementById('configStatus');

const tauriApi = window.__TAURI__;
const invoke = tauriApi?.core?.invoke;
const listen = tauriApi?.event?.listen;

let isTranslating = false;

function resetOutput() {
  outputText.textContent = '翻译结果将显示在这里';
  outputText.style.color = '#999';
}

function setTranslating(value) {
  isTranslating = value;
  translateBtn.disabled = value;
}

function setConfigStatus(message, isError = false) {
  configStatus.textContent = message;
  configStatus.style.color = isError ? '#b42318' : '#666';
}

function fillConfigForm(config) {
  targetLangInput.value = config.targetLang ?? '中文';
  apiKeyInput.value = config.openaiCompatible?.apiKey ?? '';
  baseUrlInput.value = config.openaiCompatible?.baseUrl ?? 'https://api.openai.com/v1';
  modelInput.value = config.openaiCompatible?.model ?? 'gpt-4o-mini';
  timeoutInput.value = String(config.openaiCompatible?.timeoutSeconds ?? 60);
}

function readConfigForm() {
  return {
    provider: 'openai-compatible',
    targetLang: targetLangInput.value.trim() || '中文',
    openaiCompatible: {
      apiKey: apiKeyInput.value.trim() || null,
      baseUrl: baseUrlInput.value.trim(),
      model: modelInput.value.trim(),
      timeoutSeconds: Number(timeoutInput.value) || 60,
    },
  };
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

  saveConfigBtn.disabled = true;
  setConfigStatus('保存中...');

  try {
    const config = await invoke('save_app_config', { config: readConfigForm() });
    fillConfigForm(config);
    setConfigStatus('配置已保存，下一次翻译生效');
  } catch (error) {
    setConfigStatus(String(error), true);
  } finally {
    saveConfigBtn.disabled = false;
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

function renderTranslationEvent(payload) {
  switch (payload.type) {
    case 'started':
      inputText.value = payload.sourceText ?? inputText.value;
      outputText.textContent = '';
      outputText.style.color = '#333';
      setTranslating(true);
      break;
    case 'delta':
      outputText.textContent += payload.text ?? '';
      outputText.style.color = '#333';
      break;
    case 'finished':
      outputText.textContent = payload.fullText ?? outputText.textContent;
      outputText.style.color = '#333';
      setTranslating(false);
      break;
    case 'failed':
      outputText.textContent = payload.message ?? '翻译失败';
      outputText.style.color = '#b42318';
      setTranslating(false);
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

settingsBtn.addEventListener('click', () => {
  settingsPanel.classList.toggle('hidden');
});

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
  setTranslating(true);

  try {
    await invoke('start_translation', { text });
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
    setTranslating(false);
  }
});

clearBtn.addEventListener('click', () => {
  inputText.value = '';
  resetOutput();
  setTranslating(false);
});

inputText.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    translateBtn.click();
  }
});

loadAppConfig();
applyPendingSourceText();
