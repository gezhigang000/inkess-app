import MarkdownIt from 'markdown-it'
import hljs from 'highlight.js'
import DOMPurify from 'dompurify'

export function escapeHtml(str: string): string {
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

const md: MarkdownIt = new MarkdownIt({
  html: true,
  linkify: true,
  typographer: true,
  highlight(str: string, lang: string): string {
    if (lang && hljs.getLanguage(lang)) {
      try {
        return `<pre class="hljs"><code>${hljs.highlight(str, { language: lang, ignoreIllegals: true }).value}</code></pre>`
      } catch (_) { /* ignore */ }
    }
    return `<pre class="hljs"><code>${escapeHtml(str)}</code></pre>`
  },
})

export function renderMarkdown(source: string): string {
  const raw = md.render(source)
  return DOMPurify.sanitize(raw, {
    ADD_TAGS: ['pre', 'code'],
    ADD_ATTR: ['class'],
  })
}
