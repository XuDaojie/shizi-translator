// 纯 ES module，三页共用。照 translate-card-sync.js 先例：无依赖。
// 测试与 settings 页通过注入 deps 或 @public alias 引入。

const LEVELS = { error: 0, warn: 1, info: 2, debug: 3 }
const BUFFER_LIMIT = 1000
const FLUSH_COUNT = 50
const FLUSH_INTERVAL_MS = 2000

function defaultDeps() {
  const tauri = (typeof window !== 'undefined' && window.__TAURI__) || {}
  return {
    invoke: tauri?.core?.invoke,
    now: () => new Date().toISOString(),
    addEventListener: typeof window !== 'undefined' ? window.addEventListener.bind(window) : undefined,
    setTimeout: typeof window !== 'undefined' ? window.setTimeout.bind(window) : undefined,
    clearTimeout: typeof window !== 'undefined' ? window.clearTimeout.bind(window) : undefined,
    visibilityState: typeof document !== 'undefined' ? () => document.visibilityState : () => 'visible',
  }
}

export function redactText(text, level) {
  const full = typeof text === 'string' ? text : String(text ?? '')
  if (level === 'debug') return full
  const len = full.length
  const head = full.slice(0, 20)
  return `[len=${len}] ${head}...`
}

/** 截断缓冲：超 limit 丢弃最旧（FIFO）。导出供单测。 */
export function clampBuffer(buffer, limit) {
  if (buffer.length > limit) buffer.splice(0, buffer.length - limit)
}

export function createLogger(source, deps) {
  const d = { ...defaultDeps(), ...(deps || {}) }
  let level = 'info'
  const buffer = []
  let flushTimer = null
  let flushing = false

  function shouldLog(msgLevel) {
    return (LEVELS[msgLevel] ?? 2) <= (LEVELS[level] ?? 2)
  }

  function enqueue(entry) {
    buffer.push(entry)
    clampBuffer(buffer, BUFFER_LIMIT)
    if (buffer.length >= FLUSH_COUNT) {
      flush()
    } else if (!flushTimer && d.setTimeout) {
      flushTimer = d.setTimeout(flush, FLUSH_INTERVAL_MS)
    }
  }

  // 成功才移除已提交条目（splice 在 then 里）；失败重试一次，仍失败丢弃该批，
  // buffer 保留剩余条目继续累积。flushing 锁防止并发 flush 重复提交。
  function flush() {
    if (flushTimer && d.clearTimeout) { d.clearTimeout(flushTimer); flushTimer = null }
    if (buffer.length === 0 || !d.invoke || flushing) return Promise.resolve()
    flushing = true
    const batch = buffer.slice(0, FLUSH_COUNT)
    return Promise.resolve(d.invoke('write_frontend_log', { entries: batch }))
      .then(() => { buffer.splice(0, batch.length); flushing = false })
      .catch(() => Promise.resolve(d.invoke('write_frontend_log', { entries: batch }))
        .then(() => { buffer.splice(0, batch.length); flushing = false })
        .catch(() => { flushing = false }))
  }

  function log(msgLevel, message, meta) {
    if (!shouldLog(msgLevel)) return
    enqueue({
      level: msgLevel,
      message: typeof message === 'string' ? message : String(message),
      timestamp: d.now(),
      source,
      meta: meta ?? undefined,
    })
  }

  if (d.addEventListener) {
    d.addEventListener('visibilitychange', () => {
      if (d.visibilityState && d.visibilityState() === 'hidden') flush()
    })
    d.addEventListener('beforeunload', flush)
  }

  return {
    get level() { return level },
    setLevel(newLevel) { level = newLevel },
    debug: (msg, meta) => log('debug', msg, meta),
    info: (msg, meta) => log('info', msg, meta),
    warn: (msg, meta) => log('warn', msg, meta),
    error: (msg, meta) => log('error', msg, meta),
    redactText: (text) => redactText(text, level),
    flush,
  }
}
