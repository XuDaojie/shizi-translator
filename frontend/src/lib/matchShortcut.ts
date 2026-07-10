/**
 * 将 ShortcutRecorder / 后端配置中的组合键字符串（如 `Ctrl+,`、`Alt+D`）
 * 与浏览器 KeyboardEvent 匹配。仅用于程序快捷键（窗口聚焦时）。
 */
export function matchShortcutKeys(keys: string, e: KeyboardEvent): boolean {
  const raw = keys.trim()
  if (!raw) return false

  const parts = raw.split('+').map((p) => p.trim()).filter(Boolean)
  if (parts.length === 0) return false

  let wantCtrl = false
  let wantAlt = false
  let wantShift = false
  let wantMeta = false
  let keyPart = ''

  for (const part of parts) {
    const lower = part.toLowerCase()
    if (lower === 'ctrl' || lower === 'control' || part === '⌃') {
      wantCtrl = true
    } else if (lower === 'alt' || lower === 'option' || part === '⌥') {
      wantAlt = true
    } else if (lower === 'shift' || part === '⇧') {
      wantShift = true
    } else if (lower === 'meta' || lower === 'cmd' || lower === 'command' || part === '⌘') {
      wantMeta = true
    } else {
      keyPart = part
    }
  }

  if (!keyPart) return false
  if (!!e.ctrlKey !== wantCtrl) return false
  if (!!e.altKey !== wantAlt) return false
  if (!!e.shiftKey !== wantShift) return false
  if (!!e.metaKey !== wantMeta) return false

  return eventKeyMatches(keyPart, e)
}

function eventKeyMatches(configured: string, e: KeyboardEvent): boolean {
  const conf = configured.trim()
  const confLower = conf.toLowerCase()
  const eventKey = e.key
  const eventCode = e.code

  // 逗号：部分布局下 Ctrl+, 的 key 会变成 "<" / "Unidentified"，以 code 为准
  if (conf === ',' || confLower === 'comma') {
    return eventCode === 'Comma' || eventKey === ','
  }

  // 单字符：与 e.key 忽略大小写比较
  if (conf.length === 1) {
    return eventKey.toLowerCase() === confLower
  }

  // 功能键 / 命名键
  if (eventKey.toLowerCase() === confLower) return true
  if (eventCode.toLowerCase() === confLower) return true

  // Space
  if (confLower === 'space' && (eventKey === ' ' || eventCode === 'Space')) return true

  return false
}
