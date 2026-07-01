const targetLangInput = document.getElementById('targetLangInput');
const providerSelect = document.getElementById('providerSelect');
const openaiSettings = document.getElementById('openaiSettings');
const claudeSettings = document.getElementById('claudeSettings');
const apiKeyInput = document.getElementById('apiKeyInput');
const baseUrlInput = document.getElementById('baseUrlInput');
const modelInput = document.getElementById('modelInput');
const timeoutInput = document.getElementById('timeoutInput');
const claudeApiKeyInput = document.getElementById('claudeApiKeyInput');
const claudeBaseUrlInput = document.getElementById('claudeBaseUrlInput');
const claudeModelInput = document.getElementById('claudeModelInput');
const claudeTimeoutInput = document.getElementById('claudeTimeoutInput');
const claudeEnableThinkingInput = document.getElementById('claudeEnableThinkingInput');
const popupPrecreateInput = document.getElementById('popupPrecreateInput');
const overlayPrecreateInput = document.getElementById('overlayPrecreateInput');
const saveConfigBtn = document.getElementById('saveConfigBtn');
const configStatus = document.getElementById('configStatus');

const invoke = window.__TAURI__?.core?.invoke;

function toggleProviderSettings() {
  const provider = providerSelect.value;
  openaiSettings.classList.toggle('hidden', provider !== 'openai-compatible');
  claudeSettings.classList.toggle('hidden', provider !== 'claude');
}

function setConfigStatus(message, isError = false) {
  configStatus.textContent = message;
  configStatus.style.color = isError ? '#b42318' : '#666';
}

function fillConfigForm(config) {
  targetLangInput.value = config.targetLang ?? '中文';
  providerSelect.value = config.provider ?? 'openai-compatible';
  apiKeyInput.value = config.openaiCompatible?.apiKey ?? '';
  baseUrlInput.value = config.openaiCompatible?.baseUrl ?? 'https://api.openai.com/v1';
  modelInput.value = config.openaiCompatible?.model ?? 'gpt-4o-mini';
  timeoutInput.value = String(config.openaiCompatible?.timeoutSeconds ?? 60);
  claudeApiKeyInput.value = config.claude?.apiKey ?? '';
  claudeBaseUrlInput.value = config.claude?.baseUrl ?? 'https://api.anthropic.com';
  claudeModelInput.value = config.claude?.model ?? 'claude-haiku-4-5';
  claudeTimeoutInput.value = String(config.claude?.timeoutSeconds ?? 60);
  claudeEnableThinkingInput.checked = config.claude?.enableThinking ?? false;
  popupPrecreateInput.checked = config.popupPrecreate ?? true;
  overlayPrecreateInput.checked = config.overlayPrecreate ?? true;
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
    popupPrecreate: popupPrecreateInput.checked,
    overlayPrecreate: overlayPrecreateInput.checked,
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
    const strategyChanged = config.popupPrecreate !== popupPrecreateInput.checked
      || config.overlayPrecreate !== overlayPrecreateInput.checked;
    const msg = strategyChanged
      ? '配置已保存，窗口策略切换需重启应用生效'
      : '配置已保存，下一次翻译生效';
    setConfigStatus(msg);
  } catch (error) {
    setConfigStatus(String(error), true);
  } finally {
    saveConfigBtn.disabled = false;
    saveConfigBtn.textContent = '保存配置';
  }
}

providerSelect.addEventListener('change', toggleProviderSettings);
saveConfigBtn.addEventListener('click', saveAppConfig);

loadAppConfig();
