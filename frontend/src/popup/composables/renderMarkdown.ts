import { marked } from 'marked'
import DOMPurify from 'isomorphic-dompurify'

const ALLOWED_TAGS = [
  'p', 'br', 'strong', 'em', 'b', 'i', 'code', 'pre',
  'h1', 'h2', 'h3',
  'ul', 'ol', 'li',
  'a', 'blockquote', 'hr',
]

const ALLOWED_ATTR = ['href', 'title', 'class']

marked.setOptions({
  gfm: true,
  breaks: false,
})

/** Markdown 源 → 消毒后的 HTML。空串返回 ''。 */
export function renderMarkdownToHtml(source: string): string {
  if (!source || !source.trim()) return ''
  try {
    const raw = marked.parse(source, { async: false }) as string
    return DOMPurify.sanitize(raw, {
      ALLOWED_TAGS,
      ALLOWED_ATTR,
      ALLOWED_URI_REGEXP: /^(?:(?:https?|mailto):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i,
      ALLOW_DATA_ATTR: false,
    })
  } catch {
    return escapeAsParagraph(source)
  }
}

/** 朗读用：去掉标记后的纯文本（保留换行）。 */
export function plainTextFromMarkdown(source: string): string {
  if (!source || !source.trim()) return ''
  const html = renderMarkdownToHtml(source)
  if (!html) return ''
  return htmlToPlainText(html)
}

function htmlToPlainText(html: string): string {
  // 块级后补换行，再剥标签
  const withBreaks = html
    .replace(/<\/(p|div|h[1-6]|li|blockquote|pre)>/gi, '</$1>\n')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/tr>/gi, '\n')
  const stripped = withBreaks.replace(/<[^>]+>/g, '')
  return decodeBasicEntities(stripped)
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

function decodeBasicEntities(text: string): string {
  return text
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
}

function escapeAsParagraph(text: string): string {
  const escaped = text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  return `<p>${escaped}</p>`
}

export type OpenUrlFn = (url: string) => void | Promise<void>

type ClickTarget = {
  closest?: (selector: string) => { getAttribute?: (name: string) => string | null } | null
}

/**
 * 结果正文链接委托：仅 http(s) 调用 openUrl 并 preventDefault。
 * 供弹窗 / 历史结果卡共用。参数形态兼容 MouseEvent，便于单测。
 */
export async function handleMarkdownLinkClick(
  event: { target: ClickTarget | EventTarget | null; preventDefault: () => void },
  openUrl: OpenUrlFn,
): Promise<boolean> {
  const target = event.target as ClickTarget | null
  const anchor = target?.closest?.('a[href]')
  if (!anchor?.getAttribute) return false
  const href = anchor.getAttribute('href')?.trim() ?? ''
  if (!/^https?:\/\//i.test(href)) return false
  event.preventDefault()
  await openUrl(href)
  return true
}
