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
const resultsList = document.getElementById('resultsList');
const statusDot = document.getElementById('statusDot');
const statusText = document.getElementById('statusText');
const statusAction = document.getElementById('statusAction');
const charCount = document.getElementById('charCount');
const toastEl = document.getElementById('toast');

let isTranslating = false;
let currentBatchId = null;
let pinned = false;
const resultCards = new Map();
let runtimeConfig = { autoCopy: false, defaultSourceLang: 'auto', targetLang: '中文' };
let copiedBatchId = null;
let currentSourceType = null;

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
  if (!sourceText.value.trim()) {
    resultCards.forEach(function (card) { card.el.classList.add('collapsed'); });
    adjustHeight();
  }
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
// 按 payload.serviceType（渠道 id）匹配；未匹配 fallback 取 serviceName 首字，灰底。
const ENGINE_META = {
  openai: { color: '#10A37F', letter: 'O' },
  deepseek: { color: '#4D6BFE', letter: 'D' },
  zhipu: { color: '#3B5BFE', letter: 'Z' },
  claude: { color: '#D97757', letter: 'C' },
  mock: { color: '#94918A', letter: 'M' },
};

function engineIcon(serviceType, serviceName) {
  const meta = ENGINE_META[serviceType];
  const color = meta ? meta.color : '#94918A';
  const letter = meta
    ? meta.letter
    : ((serviceName || '?').trim().charAt(0).toUpperCase() || '?');
  return (
    '<rect width="20" height="20" rx="5" fill="' + color + '"/>' +
    '<text x="10" y="14.5" text-anchor="middle" font-size="12" font-weight="700" fill="#fff" ' +
    'font-family="Segoe UI, system-ui, sans-serif">' + letter + '</text>'
  );
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

/* === batchId 辅助 === */
function batchIdFromSession(sessionId) {
  return typeof sessionId === 'string' ? sessionId.split(':')[0] : null;
}

/* === 结果卡片 === */
function getCard(payload) {
  const id = payload.serviceInstanceId ?? 'default';
  let existing = resultCards.get(id);
  if (existing) return existing;

  const card = document.createElement('div');
  card.className = 'result-card';
  if (!sourceText.value.trim()) card.classList.add('collapsed');
  card.dataset.serviceInstanceId = id;

  const displayName = payload.serviceName ?? '翻译';

  card.innerHTML = [
    '<div class="result-card-header">',
    '  <svg class="result-engine-icon" viewBox="0 0 20 20"></svg>',
    '  <span class="result-engine-name">' + displayName + '</span>',
    '  <button class="result-collapse-btn" title="折叠">',
    '    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>',
    '  </button>',
    '</div>',
    '<div class="result-card-body">',
    '  <div class="result-card-body-inner">',
    '    <div class="result-text"></div>',
    '    <div class="result-actions" style="visibility:hidden">',
    '      <button class="result-action-btn speak-btn" title="朗读翻译">',
    '        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07"/></svg>',
    '      </button>',
    '      <button class="result-action-btn copy-btn" title="复制翻译">',
    '        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>',
    '      </button>',
    '      <span class="result-tokens" style="display:none">',
    '        <span class="tok tok-input">',
    '          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/></svg>',
    '          <span class="tok-value">0</span>',
    '        </span>',
    '        <span class="tok-sep"></span>',
    '        <span class="tok tok-output">',
    '          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><polyline points="19 12 12 19 5 12"/></svg>',
    '          <span class="tok-value">0</span>',
    '        </span>',
    '      </span>',
    '    </div>',
    '  </div>',
    '</div>',
  ].join('\n');

  card.querySelector('.result-engine-icon').innerHTML = engineIcon(
    payload.serviceType,
    payload.serviceName,
  );

  const text = card.querySelector('.result-text');
  const actions = card.querySelector('.result-actions');
  const tokens = card.querySelector('.result-tokens');
  const inputTokens = tokens.querySelector('.tok-input .tok-value');
  const outputTokens = tokens.querySelector('.tok-output .tok-value');

  const header = card.querySelector('.result-card-header');
  const collapseBtn = card.querySelector('.result-collapse-btn');
  header.addEventListener('click', (e) => {
    if (e.target.closest('.result-collapse-btn')) return;
    card.classList.toggle('collapsed');
    adjustHeight();
  });
  collapseBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    card.classList.toggle('collapsed');
    adjustHeight();
  });

  const copyBtn = card.querySelector('.copy-btn');
  copyBtn.addEventListener('click', () => copyText(text.textContent, copyBtn));

  const speakBtn = card.querySelector('.speak-btn');
  speakBtn.addEventListener('click', () => speakText(text.textContent, 'zh-CN'));

  resultsList.appendChild(card);

  const ref = { el: card, text, actions, tokens, inputTokens, outputTokens, status: 'pending' };
  resultCards.set(id, ref);
  return ref;
}

/* === 流式光标 === */
function setStreamCursor(card, visible) {
  const existing = card.text.querySelector('.stream-cursor');
  if (existing) existing.remove();
  if (visible) {
    const cursor = document.createElement('span');
    cursor.className = 'stream-cursor';
    card.text.appendChild(cursor);
  }
}

function scrollToBottom(card) {
  card.text.scrollTop = card.text.scrollHeight;
}

/* === 状态栏 === */
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

function updateBatchStatus() {
  const cards = Array.from(resultCards.values());
  if (cards.length === 0) return;
  const allFinished = cards.every(c => c.status === 'finished');
  const allFailed = cards.every(c => c.status === 'failed' || c.status === 'cancelled');
  const anyTranslating = cards.some(c => c.status === 'translating');

  if (allFinished) {
    if (runtimeConfig.autoCopy && copiedBatchId !== currentBatchId) {
      for (var i = 0; i < cards.length; i++) {
        if (cards[i].text.textContent.trim()) {
          copiedBatchId = currentBatchId;
          navigator.clipboard.writeText(cards[i].text.textContent).catch(function () {});
          break;
        }
      }
    }
    isTranslating = false;
    currentBatchId = null;
    setSourceBadge(null);
    setStatus({ text: '翻译完成', loading: false, action: { label: '重试', onClick: retryTranslation } });
  } else if (allFailed) {
    isTranslating = false;
    currentBatchId = null;
    setStatus({ text: '翻译失败', loading: false, action: { label: '重试', onClick: retryTranslation } });
  } else if (anyTranslating) {
    setStatus({ text: '翻译中…', loading: true, action: { label: '取消', onClick: cancelTranslation } });
  } else {
    isTranslating = false;
    currentBatchId = null;
    setSourceBadge(null);
    setStatus({ text: '部分完成', loading: false, action: { label: '重试', onClick: retryTranslation } });
  }
}

/* === 翻译事件渲染 === */
function renderTranslationEvent(payload) {
  switch (payload.type) {
    case 'started': {
      const batchId = batchIdFromSession(payload.sessionId);
      const isNewBatch = batchId !== currentBatchId;
      if (isNewBatch) {
        currentBatchId = batchId;
        resultCards.forEach(function (c) {
          c.status = 'pending';
          c.text.textContent = '';
          c.text.style.color = '';
          c.actions.style.visibility = 'hidden';
          c.tokens.style.display = 'none';
          c.el.classList.remove('failed', 'cancelled');
        });
        sourceText.value = payload.sourceText ?? sourceText.value;
        autoResize();
        updateCharCount();
        setSourceBadge(payload.sourceType);
        currentSourceType = payload.sourceType;
        copiedBatchId = null;
        isTranslating = true;
        setStatus({ text: '翻译中…', loading: true, action: { label: '取消', onClick: cancelTranslation } });
      }
      const card = getCard(payload);
      card.status = 'translating';
      card.text.textContent = '';
      card.text.style.color = '';
      card.actions.style.visibility = 'hidden';
      card.tokens.style.display = 'none';
      card.el.classList.remove('collapsed');
      card.el.classList.remove('failed', 'cancelled');
      setStreamCursor(card, true);
      break;
    }
    case 'delta': {
      if (batchIdFromSession(payload.sessionId) !== currentBatchId) return;
      const card = resultCards.get(payload.serviceInstanceId ?? 'default');
      if (!card) return;
      card.text.appendChild(document.createTextNode(payload.text ?? ''));
      setStreamCursor(card, true);
      scrollToBottom(card);
      break;
    }
    case 'finished': {
      if (batchIdFromSession(payload.sessionId) !== currentBatchId) return;
      const card = resultCards.get(payload.serviceInstanceId ?? 'default');
      if (!card) return;
      card.text.textContent = payload.fullText ?? card.text.textContent;
      card.text.style.color = '';
      setStreamCursor(card, false);
      if (payload.usage) {
        card.inputTokens.textContent = payload.usage.inputTokens;
        card.outputTokens.textContent = payload.usage.outputTokens;
        card.tokens.style.display = '';
      } else {
        card.tokens.style.display = 'none';
      }
      card.actions.style.visibility = 'visible';
      card.status = 'finished';
      scrollToBottom(card);
      updateBatchStatus();
      if (currentSourceType === 'ocrText') {
        try {
          var SETTINGS_KEY = 'app:settings:v1';
          var raw = localStorage.getItem(SETTINGS_KEY);
          var st = raw ? JSON.parse(raw) : {};
          var history = Array.isArray(st.ocrHistory) ? st.ocrHistory : [];
          history.unshift({
            id: 'hist-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 8),
            timestamp: new Date().toISOString(),
            sourceLang: runtimeConfig.defaultSourceLang || 'auto',
            targetLang: runtimeConfig.targetLang || '中文',
            source: sourceText.value || '',
            translation: card.text.textContent || '',
            serviceInstanceId: payload.serviceInstanceId || '',
          });
          var limit = Math.max(1, Number(st.translation && st.translation.historyLimit ? st.translation.historyLimit : 500));
          st.ocrHistory = history.slice(0, limit);
          localStorage.setItem(SETTINGS_KEY, JSON.stringify(st));
        } catch (e) {
          // 静默失败，不影响翻译流程
        }
      }
      break;
    }
    case 'failed': {
      if (batchIdFromSession(payload.sessionId) !== currentBatchId) return;
      const card = resultCards.get(payload.serviceInstanceId ?? 'default');
      if (!card) return;
      card.text.textContent = payload.message ?? '翻译失败';
      card.text.style.color = 'var(--danger)';
      setStreamCursor(card, false);
      card.actions.style.visibility = 'hidden';
      card.tokens.style.display = 'none';
      card.el.classList.add('failed');
      card.status = 'failed';
      updateBatchStatus();
      break;
    }
    case 'cancelled': {
      if (batchIdFromSession(payload.sessionId) !== currentBatchId) return;
      const card = resultCards.get(payload.serviceInstanceId ?? 'default');
      if (!card) return;
      card.text.appendChild(document.createTextNode('\n[已取消]'));
      card.text.style.color = 'var(--fg-3)';
      setStreamCursor(card, false);
      card.el.classList.add('cancelled');
      card.status = 'cancelled';
      updateBatchStatus();
      break;
    }
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

pinBtn.addEventListener('click', togglePin);
favBtn.addEventListener('click', toggleFav);
ocrBtn.addEventListener('click', triggerOcr);
bookmarkBtn.addEventListener('click', () => showToast('功能开发中'));
settingsBtn.addEventListener('click', openSettings);
speakSourceBtn.addEventListener('click', () => speakText(sourceText.value, 'en-US'));
copySourceBtn.addEventListener('click', () => copyText(sourceText.value, copySourceBtn));
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

/* === 弹窗打开时预建所有启用服务的占位卡片 === */
async function initCards() {
  if (!invoke) return;
  let config;
  try { config = await invoke("get_app_config"); } catch { return; }
  if (!config.services || config.services.length === 0) return;
  runtimeConfig.autoCopy = Boolean(config.autoCopy);
  runtimeConfig.defaultSourceLang = config.defaultSourceLang || 'auto';
  runtimeConfig.targetLang = config.targetLang || '中文';
  const enabled = config.services.filter(function (s) { return s.enabled; });
  if (enabled.length === 0) return;
  for (var i = 0; i < enabled.length; i++) {
    var svc = enabled[i];
    var payload = {
      serviceInstanceId: svc.id,
      serviceType: svc.serviceType,
      serviceName: svc.name,
    };
    getCard(payload);
  }
}

const resizeObserver = new ResizeObserver(adjustHeight);
resizeObserver.observe(popupEl);

/* === 初始化 === */
initMaxHeight();
initCards();
autoResize();
updateCharCount();
applyPendingSourceText();
