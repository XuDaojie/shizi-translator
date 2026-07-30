import { describe, expect, it, vi } from 'vitest'
import {
  handleMarkdownLinkClick,
  plainTextFromMarkdown,
  renderMarkdownToHtml,
} from './renderMarkdown'

describe('renderMarkdownToHtml', () => {
  it('渲染粗体与列表', () => {
    const html = renderMarkdownToHtml('**你好**\n\n- 一项\n- 二项')
    expect(html).toContain('<strong>')
    expect(html).toContain('你好')
    expect(html).toMatch(/<ul>/i)
    expect(html).toContain('一项')
  })

  it('渲染行内代码与代码块', () => {
    const html = renderMarkdownToHtml('用 `code` 与\n\n```\nconst x = 1\n```')
    expect(html).toContain('<code>')
    expect(html).toContain('<pre>')
    expect(html).toContain('const x = 1')
  })

  it('剥离 script 与危险 HTML', () => {
    const html = renderMarkdownToHtml('<script>alert(1)</script>正文')
    expect(html.toLowerCase()).not.toContain('<script')
    expect(html).toContain('正文')
  })

  it('中和 javascript: 链接', () => {
    const html = renderMarkdownToHtml('[x](javascript:alert(1))')
    expect(html.toLowerCase()).not.toContain('javascript:')
  })

  it('保留 https 链接', () => {
    const html = renderMarkdownToHtml('[文档](https://example.com/docs)')
    expect(html).toContain('href="https://example.com/docs"')
    expect(html).toContain('文档')
  })

  it('空串返回空', () => {
    expect(renderMarkdownToHtml('')).toBe('')
    expect(renderMarkdownToHtml('   ')).toBe('')
  })

  it('纯文本无崩溃', () => {
    const html = renderMarkdownToHtml('普通译文没有标记')
    expect(html).toContain('普通译文没有标记')
    expect(html.toLowerCase()).not.toContain('<script')
  })
})

describe('plainTextFromMarkdown', () => {
  it('朗读时去掉粗体标记', () => {
    expect(plainTextFromMarkdown('**hi**')).toBe('hi')
  })

  it('保留列表项文字', () => {
    const text = plainTextFromMarkdown('- 甲\n- 乙')
    expect(text).toContain('甲')
    expect(text).toContain('乙')
  })

  it('空串', () => {
    expect(plainTextFromMarkdown('')).toBe('')
  })
})

describe('handleMarkdownLinkClick', () => {
  it('http(s) 链接调用 openUrl 并 preventDefault', async () => {
    const openUrl = vi.fn(async () => {})
    const anchor = {
      getAttribute: (name: string) => (name === 'href' ? 'https://example.com/path' : null),
    }
    const target = {
      closest: (sel: string) => (sel === 'a[href]' ? anchor : null),
    }
    const e = {
      target,
      preventDefault: vi.fn(),
    }

    await handleMarkdownLinkClick(e, openUrl)
    expect(e.preventDefault).toHaveBeenCalled()
    expect(openUrl).toHaveBeenCalledWith('https://example.com/path')
  })

  it('非链接点击不处理', async () => {
    const openUrl = vi.fn(async () => {})
    const e = {
      target: { closest: () => null },
      preventDefault: vi.fn(),
    }
    await handleMarkdownLinkClick(e, openUrl)
    expect(e.preventDefault).not.toHaveBeenCalled()
    expect(openUrl).not.toHaveBeenCalled()
  })
})
