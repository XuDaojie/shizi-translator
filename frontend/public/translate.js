const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;
const getCurrentWindow = window.__TAURI__?.window?.getCurrentWindow;

const popupEl = document.getElementById('popup');
const sourceText = document.getElementById('sourceText');
const speakSourceBtn = document.getElementById('speakSourceBtn');
const copySourceBtn = document.getElementById('copySourceBtn');
const sourceBadge = document.getElementById('sourceBadge');
const pinBtn = document.getElementById('pinBtn');
const favBtn = document.getElementById('favBtn');
const ocrBtn = document.getElementById('ocrBtn');
const bookmarkBtn = document.getElementById('bookmarkBtn');
const settingsBtn = document.getElementById('settingsBtn');
const langSource = document.getElementById('langSource');
const langSwap = document.getElementById('langSwap');
const langTarget = document.getElementById('langTarget');
const resultCard = document.getElementById('resultCard');
const resultHeader = document.getElementById('resultHeader');
const resultEngineIcon = document.getElementById('resultEngineIcon');
const resultEngineName = document.getElementById('resultEngineName');
const collapseBtn = document.getElementById('collapseBtn');
const resultText = document.getElementById('resultText');
const resultActions = document.getElementById('resultActions');
const speakResultBtn = document.getElementById('speakResultBtn');
const copyResultBtn = document.getElementById('copyResultBtn');
const resultTokens = document.getElementById('resultTokens');
const tokInputValue = resultTokens.querySelector('.tok-input .tok-value');
const tokOutputValue = resultTokens.querySelector('.tok-output .tok-value');
const statusDot = document.getElementById('statusDot');
const statusText = document.getElementById('statusText');
const statusAction = document.getElementById('statusAction');
const charCount = document.getElementById('charCount');
const toastEl = document.getElementById('toast');

let isTranslating = false;
let currentSessionId = null;
let pinned = false;

/* === Toast === */
let toastTimer = null;
function showToast(msg) {
  toastEl.textContent = msg;
  toastEl.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastEl.classList.remove('show'), 1800);
}

/* === 原文区 === */
function autoResize() {
  sourceText.style.height = 'auto';
  sourceText.style.height = sourceText.scrollHeight + 'px';
}
function updateCharCount() {
  charCount.textContent = `${sourceText.value.length} 字`;
}
sourceText.addEventListener('input', () => {
  autoResize();
  updateCharCount();
});
sourceText.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    startManualTranslation();
  }
});

/* === 朗读 === */
function speakText(text, lang) {
  if (!('speechSynthesis' in window)) {
    showToast('当前浏览器不支持语音朗读');
    return;
  }
  window.speechSynthesis.cancel();
  const utter = new SpeechSynthesisUtterance(text);
  utter.lang = lang;
  utter.rate = 0.95;
  window.speechSynthesis.speak(utter);
}

/* === 复制 === */
function copyText(text, btn) {
  navigator.clipboard.writeText(text).then(() => {
    btn.classList.add('copied');
    showToast('已复制到剪贴板');
    setTimeout(() => btn.classList.remove('copied'), 1500);
  }).catch(() => {
    showToast('复制失败');
  });
}

/* === 引擎图标/名映射 === */
const ENGINE_META = {
  'openai-compatible': {
    icon: '<svg viewBox="0 0 20 20"><rect width="20" height="20" rx="5" fill="#10A37F"/><circle cx="10" cy="10" r="6" fill="none" stroke="#fff" stroke-width="1.2"/><path d="M7.5 10c0-1.38 1.12-2.5 2.5-2.5s2.5 1.12 2.5 2.5" stroke="#fff" stroke-width="1.2" fill="none" stroke-linecap="round"/></svg>',
    name: 'OpenAI 翻译',
  },
  'claude': {
    icon: '<svg viewBox="0 0 20 20"><rect width="20" height="20" rx="5" fill="#D97757"/><text x="10" y="14.5" text-anchor="middle" font-size="12" font-weight="700" fill="#fff" font-family="Segoe UI, system-ui, sans-serif">C</text></svg>',
    name: 'Claude 翻译',
  },
  'mock': {
    icon: '<svg viewBox="0 0 20 20"><rect width="20" height="20" rx="5" fill="#94918A"/><text x="10" y="14.5" text-anchor="middle" font-size="12" font-weight="700" fill="#fff" font-family="Segoe UI, system-ui, sans-serif">M</text></svg>',
    name: 'Mock 翻译',
  },
};

async function loadEngineMeta() {
  if (!invoke) return;
  try {
    const config = await invoke('get_app_config');
    const meta = ENGINE_META[config.provider] ?? ENGINE_META['openai-compatible'];
    resultEngineIcon.innerHTML = meta.icon;
    resultEngineName.textContent = meta.name;
  } catch (error) {
    showToast(String(error));
  }
}

/* === 来源徽章 === */
function setSourceBadge(sourceType) {
  switch (sourceType) {
    case 'selectedText':
      sourceBadge.textContent = '来自划词';
      break;
    case 'ocrText':
      sourceBadge.textContent = '来自 OCR';
      break;
    default:
      sourceBadge.textContent = '';
      break;
  }
}

/* === 翻译事件渲染 === */
function getSessionId(payload) {
  return payload?.sessionId ?? null;
}
function shouldHandleSessionEvent(payload) {
  const sessionId = getSessionId(payload);
  return !currentSessionId || !sessionId || sessionId === currentSessionId;
}

function setStatus({ text, loading, action }) {
  statusText.textContent = text;
  statusDot.classList.toggle('loading', loading);
  if (action) {
    statusAction.textContent = action.label;
    statusAction.style.display = '';
    statusAction.onclick = action.onClick;
  } else {
    statusAction.style.display = 'none';
    statusAction.onclick = null;
  }
}

function setStreamCursor(visible) {
  const existing = resultText.querySelector('.stream-cursor');
  if (existing) existing.remove();
  if (visible) {
    const cursor = document.createElement('span');
    cursor.className = 'stream-cursor';
    resultText.appendChild(cursor);
  }
}

function scrollResultToBottom() {
  resultText.scrollTop = resultText.scrollHeight;
}

function renderTranslationEvent(payload) {
  switch (payload.type) {
    case 'started':
      currentSessionId = getSessionId(payload);
      sourceText.value = payload.sourceText ?? sourceText.value;
      autoResize();
      updateCharCount();
      setSourceBadge(payload.sourceType);
      resultText.textContent = '';
      resultText.style.color = '';
      resultActions.style.visibility = 'hidden';
      resultTokens.style.display = 'none';
      setStreamCursor(true);
      isTranslating = true;
      setStatus({
        text: '翻译中…',
        loading: true,
        action: { label: '取消', onClick: cancelTranslation },
      });
      break;
    case 'delta':
      if (!shouldHandleSessionEvent(payload)) return;
      resultText.appendChild(document.createTextNode(payload.text ?? ''));
      setStreamCursor(true);
      scrollResultToBottom();
      break;
    case 'finished':
      if (!shouldHandleSessionEvent(payload)) return;
      resultText.textContent = payload.fullText ?? resultText.textContent;
      resultText.style.color = '';
      setStreamCursor(false);
      if (payload.usage) {
        tokInputValue.textContent = payload.usage.inputTokens;
        tokOutputValue.textContent = payload.usage.outputTokens;
        resultTokens.style.display = '';
      } else {
        resultTokens.style.display = 'none';
      }
      resultActions.style.visibility = 'visible';
      currentSessionId = null;
      isTranslating = false;
      setSourceBadge(null);
      setStatus({
        text: '翻译完成',
        loading: false,
        action: { label: '重试', onClick: retryTranslation },
      });
      scrollResultToBottom();
      break;
    case 'failed':
      if (currentSessionId && !shouldHandleSessionEvent(payload)) return;
      resultText.textContent = payload.message ?? '翻译失败';
      resultText.style.color = 'var(--danger)';
      setStreamCursor(false);
      resultActions.style.visibility = 'hidden';
      resultTokens.style.display = 'none';
      currentSessionId = null;
      isTranslating = false;
      setSourceBadge(null);
      setStatus({
        text: '翻译失败',
        loading: false,
        action: payload.retryable !== false
          ? { label: '重试', onClick: retryTranslation }
          : null,
      });
      break;
    case 'cancelled':
      if (!shouldHandleSessionEvent(payload)) return;
      resultText.appendChild(document.createTextNode('\n[已取消]'));
      resultText.style.color = 'var(--fg-3)';
      setStreamCursor(false);
      currentSessionId = null;
      isTranslating = false;
      setSourceBadge(null);
      setStatus({
        text: '已取消',
        loading: false,
        action: { label: '重试', onClick: retryTranslation },
      });
      break;
    default:
      break;
  }
  adjustHeight();
}

if (listen) {
  listen('translation:event', (event) => {
    renderTranslationEvent(event.payload);
  });
}

/* === 翻译触发 === */
async function startManualTranslation() {
  if (isTranslating) return;
  const text = sourceText.value.trim();
  if (!text) {
    showToast('请输入要翻译的文本');
    return;
  }
  if (!invoke) {
    showToast('Tauri API 未就绪，请在桌面应用中运行');
    return;
  }
  try {
    await invoke('start_translation', { text });
  } catch (error) {
    showToast(String(error));
  }
}

async function cancelTranslation() {
  if (!invoke) return;
  try {
    await invoke('cancel_translation');
  } catch (error) {
    showToast(String(error));
  }
}

async function retryTranslation() {
  if (isTranslating) return;
  if (!invoke) {
    showToast('Tauri API 未就绪');
    return;
  }
  try {
    await invoke('retry_translation');
  } catch (error) {
    showToast(String(error));
  }
}

/* === 工具栏按钮 === */
async function togglePin() {
  if (!getCurrentWindow) {
    showToast('窗口 API 未就绪');
    return;
  }
  pinned = !pinned;
  pinBtn.classList.toggle('active', pinned);
  try {
    await getCurrentWindow().setAlwaysOnTop(pinned);
    showToast(pinned ? '窗口已固定' : '取消固定');
  } catch (error) {
    pinned = !pinned;
    pinBtn.classList.toggle('active', pinned);
    showToast(String(error));
  }
}

function toggleFav() {
  favBtn.classList.toggle('active');
  showToast(favBtn.classList.contains('active') ? '已收藏' : '取消收藏');
}

async function triggerOcr() {
  if (!invoke) {
    showToast('Tauri API 未就绪');
    return;
  }
  try {
    await invoke('trigger_ocr_translation');
  } catch (error) {
    showToast(String(error));
  }
}

async function openSettings() {
  if (!invoke) return;
  try {
    await invoke('open_settings');
  } catch (error) {
    showToast(String(error));
  }
}

function toggleCollapse() {
  resultCard.classList.toggle('collapsed');
  adjustHeight();
}

pinBtn.addEventListener('click', togglePin);
favBtn.addEventListener('click', toggleFav);
ocrBtn.addEventListener('click', triggerOcr);
bookmarkBtn.addEventListener('click', () => showToast('功能开发中'));
settingsBtn.addEventListener('click', openSettings);
resultHeader.addEventListener('click', (e) => {
  if (e.target.closest('.result-collapse-btn')) return;
  toggleCollapse();
});
collapseBtn.addEventListener('click', (e) => {
  e.stopPropagation();
  toggleCollapse();
});
speakSourceBtn.addEventListener('click', () => speakText(sourceText.value, 'en-US'));
copySourceBtn.addEventListener('click', () => copyText(sourceText.value, copySourceBtn));
speakResultBtn.addEventListener('click', () => speakText(resultText.textContent, 'zh-CN'));
copyResultBtn.addEventListener('click', () => copyText(resultText.textContent, copyResultBtn));
langSource.addEventListener('click', () => showToast('功能开发中'));
langSwap.addEventListener('click', () => showToast('功能开发中'));
langTarget.addEventListener('click', () => showToast('功能开发中'));

/* === 待回填原文 === */
async function applyPendingSourceText() {
  if (!invoke) return;
  try {
    const text = await invoke('take_pending_source_text');
    if (text) {
      sourceText.value = text;
      autoResize();
      updateCharCount();
    }
  } catch (error) {
    showToast(String(error));
  }
}
window.addEventListener('focus', applyPendingSourceText);

/* === 高度自适应 === */
let resizeRaf = null;
let lastHeight = 0;
function adjustHeight() {
  if (resizeRaf) cancelAnimationFrame(resizeRaf);
  resizeRaf = requestAnimationFrame(() => {
    const h = popupEl.offsetHeight;
    if (h === lastHeight) return;
    lastHeight = h;
    if (getCurrentWindow) {
      getCurrentWindow()
        .setSize({ type: 'Logical', width: 420, height: h })
        .catch(() => {});
    }
  });
}
function initMaxHeight() {
  const maxPopupH = Math.floor(window.screen.availHeight * 0.8);
  popupEl.style.maxHeight = maxPopupH + 'px';
}
const resizeObserver = new ResizeObserver(adjustHeight);
resizeObserver.observe(popupEl);

/* === 初始化 === */
initMaxHeight();
autoResize();
updateCharCount();
loadEngineMeta();
applyPendingSourceText();
