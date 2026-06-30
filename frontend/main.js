const inputText = document.getElementById('inputText');
const outputText = document.getElementById('outputText');
const translateBtn = document.getElementById('translateBtn');
const clearBtn = document.getElementById('clearBtn');

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

function renderTranslationEvent(payload) {
  switch (payload.type) {
    case 'started':
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
    await invoke('start_mock_translation', { text });
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
