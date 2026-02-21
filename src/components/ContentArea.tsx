import { useState, useMemo, useRef, useEffect, useCallback, lazy, Suspense, Component, type ReactNode, type ErrorInfo } from 'react'
import { renderMarkdown } from '../lib/markdown'
import { type ThemeId } from '../lib/themes'
import { type ContentData } from '../types/content'
import { TextViewer } from './viewers/TextViewer'
import { CodeViewer } from './viewers/CodeViewer'
import { ImageViewer } from './viewers/ImageViewer'
import hljs from 'highlight.js'
import DOMPurify from 'dompurify'
import { useI18n } from '../lib/i18n'
import { MarkdownToolbar } from './MarkdownToolbar'
import { toggleInlineWrap, insertAtCursor } from '../lib/markdownFormat'

// Lazy load heavy viewers
const PdfViewer = lazy(() => import('./viewers/PdfViewer').then(m => ({ default: m.PdfViewer })))
const DocxViewer = lazy(() => import('./viewers/DocxViewer').then(m => ({ default: m.DocxViewer })))
const ExcelViewer = lazy(() => import('./viewers/ExcelViewer').then(m => ({ default: m.ExcelViewer })))

// Error Boundary for viewer components
class ViewerErrorBoundary extends Component<{ children: ReactNode; fallbackMessage?: string }, { hasError: boolean; error: string }> {
  state = { hasError: false, error: '' }
  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error: error.message }
  }
  componentDidCatch(_error: Error, _info: ErrorInfo) { /* logged via getDerivedStateFromError */ }
  render() {
    if (this.state.hasError) {
      return (
        <div style={{ padding: 40, textAlign: 'center', color: 'var(--text-3)' }}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ width: 32, height: 32, margin: '0 auto 12px', opacity: 0.5 }}>
            <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <div style={{ fontSize: 13 }}>{this.props.fallbackMessage || 'Failed to render this file'}</div>
          <div style={{ fontSize: 11, marginTop: 6, opacity: 0.6 }}>{this.state.error}</div>
        </div>
      )
    }
    return this.props.children
  }
}

interface ContentAreaProps {
  content: ContentData
  themeId: ThemeId
  editing: boolean
  onEdit: (text: string) => void
  onExport?: (format: string) => void
  onToggleEdit?: () => void
  canExport?: boolean
  canEdit?: boolean
  isReadOnly?: boolean
  currentFilePath?: string
}

export function ContentArea({ content, themeId, editing, onEdit, onExport, onToggleEdit, canExport, canEdit, isReadOnly, currentFilePath }: ContentAreaProps) {
  const { t } = useI18n()
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number } | null>(null)
  const ctxRef = useRef<HTMLDivElement>(null)

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    // Don't override context menu in editing mode (need native copy/paste)
    if (editing) return
    e.preventDefault()
    setCtxMenu({ x: e.clientX, y: e.clientY })
  }, [editing])

  useEffect(() => {
    if (!ctxMenu) return
    const close = (e: MouseEvent) => {
      if (ctxRef.current && !ctxRef.current.contains(e.target as Node)) setCtxMenu(null)
    }
    const esc = (e: KeyboardEvent) => { if (e.key === 'Escape') setCtxMenu(null) }
    document.addEventListener('mousedown', close)
    document.addEventListener('keydown', esc)
    return () => { document.removeEventListener('mousedown', close); document.removeEventListener('keydown', esc) }
  }, [ctxMenu])

  const isMarkdown = content.type === 'markdown'
  const hasText = 'text' in content

  const handleFormat = useCallback(() => {
    if (!isMarkdown || !hasText) return
    const text = (content as { text: string }).text
    const formatted = formatMarkdown(text)
    onEdit(formatted)
    setCtxMenu(null)
  }, [content, isMarkdown, hasText, onEdit])

  const loadingFallback = (
    <div style={{ padding: 40, textAlign: 'center', color: 'var(--text-3)', fontSize: 13 }}>Loading...</div>
  )

  const inner = (() => {
    switch (content.type) {
      case 'markdown':
        return <MarkdownViewer text={content.text} themeId={themeId} editing={editing} onEdit={onEdit} currentFilePath={currentFilePath} />
      case 'text':
        return <TextViewer text={content.text} editing={editing} onEdit={onEdit} />
      case 'code':
        return <CodeViewer text={content.text} language={content.language} editing={editing} onEdit={onEdit} />
      case 'html':
        return editing
          ? <CodeViewer text={content.text} language="html" editing={editing} onEdit={onEdit} />
          : <HtmlViewer html={content.text} />
      case 'image':
        return <ImageViewer src={content.src} />
      case 'pdf':
        return <Suspense fallback={loadingFallback}><PdfViewer src={content.src} data={content.data} /></Suspense>
      case 'docx':
        return <Suspense fallback={loadingFallback}><DocxViewer html={content.html} /></Suspense>
      case 'xlsx':
        return <Suspense fallback={loadingFallback}><ExcelViewer sheets={content.sheets} /></Suspense>
    }
  })()

  return (
    <div style={{ display: 'contents' }} onContextMenu={handleContextMenu}>
      <ViewerErrorBoundary>
        {inner}
      </ViewerErrorBoundary>
      {ctxMenu && (
        <div ref={ctxRef} className="ctx-menu" style={{ left: ctxMenu.x, top: ctxMenu.y }}>
          {canEdit && !isReadOnly && onToggleEdit && (
            <button className="ctx-menu-item" onClick={() => { onToggleEdit(); setCtxMenu(null) }}>
              {editing ? t('content.switchToRead') : t('content.switchToEdit')}
            </button>
          )}
          {isMarkdown && !editing && hasText && (
            <button className="ctx-menu-item" onClick={handleFormat}>
              {t('content.formatMd')}
            </button>
          )}
          {(canEdit || canExport) && <div className="ctx-menu-sep" />}
          {canExport && onExport && (
            <>
              {['PDF', 'DOCX', 'HTML', 'PPTX'].map(fmt => (
                <button key={fmt} className="ctx-menu-item" onClick={() => { onExport(fmt); setCtxMenu(null) }}>
                  {t('content.exportAs', { fmt: fmt === 'DOCX' ? 'Word' : fmt === 'PPTX' ? 'PPT' : fmt })}
                </button>
              ))}
            </>
          )}
        </div>
      )}
    </div>
  )
}

// Simple Markdown formatter: normalize spacing, headings, lists, blank lines
function formatMarkdown(text: string): string {
  const lines = text.split('\n')
  const result: string[] = []
  let prevBlank = false

  for (let i = 0; i < lines.length; i++) {
    let line = lines[i].replace(/\t/g, '    ').trimEnd()

    // Normalize heading: ensure space after #
    line = line.replace(/^(#{1,6})([^ #\n])/, '$1 $2')

    // Normalize list markers: ensure space after - or *
    line = line.replace(/^(\s*)([-*])([^ \n])/, '$1$2 $3')

    // Collapse multiple blank lines into one
    const isBlank = line.trim() === ''
    if (isBlank && prevBlank) continue

    // Ensure blank line before headings (except first line)
    if (/^#{1,6} /.test(line) && result.length > 0 && result[result.length - 1].trim() !== '') {
      result.push('')
    }

    result.push(line)
    prevBlank = isBlank
  }

  // Ensure trailing newline
  let formatted = result.join('\n')
  if (!formatted.endsWith('\n')) formatted += '\n'
  return formatted
}

function MarkdownViewer({ text, themeId, editing, onEdit, currentFilePath }: {
  text: string; themeId: ThemeId; editing: boolean; onEdit: (text: string) => void; currentFilePath?: string
}) {
  const html = useMemo(() => renderMarkdown(text), [text])
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (editing && textareaRef.current) textareaRef.current.focus()
  }, [editing])

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    const ta = textareaRef.current
    if (!ta) return
    const mod = e.metaKey || e.ctrlKey
    if (mod && e.key === 'b') {
      e.preventDefault()
      onEdit(toggleInlineWrap(ta, '**'))
    } else if (mod && e.key === 'i') {
      e.preventDefault()
      onEdit(toggleInlineWrap(ta, '*'))
    } else if (mod && e.shiftKey && (e.key === 'x' || e.key === 'X')) {
      e.preventDefault()
      onEdit(toggleInlineWrap(ta, '~~'))
    } else if (mod && e.key === 'k') {
      e.preventDefault()
      // Insert link template
      const sel = ta.value.substring(ta.selectionStart, ta.selectionEnd)
      const link = sel ? `[${sel}](url)` : '[text](url)'
      onEdit(insertAtCursor(ta, link))
    }
  }, [onEdit])

  const handlePaste = useCallback(async (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const items = e.clipboardData?.items
    if (!items) return
    for (const item of Array.from(items)) {
      if (item.type.startsWith('image/')) {
        e.preventDefault()
        if (!currentFilePath) return
        const blob = item.getAsFile()
        if (!blob) return
        const ext = item.type.split('/')[1] === 'jpeg' ? 'jpg' : item.type.split('/')[1] || 'png'
        const filename = `paste-${Date.now()}.${ext}`
        const parts = currentFilePath.replace(/\\/g, '/').split('/')
        parts.pop()
        const parentDir = parts.join('/')
        const fullPath = parentDir + '/' + filename
        try {
          const buffer = await blob.arrayBuffer()
          const bytes = Array.from(new Uint8Array(buffer))
          const { invoke } = await import('@tauri-apps/api/core')
          await invoke('write_file', { path: fullPath, contents: bytes })
          const ta = textareaRef.current
          if (ta) onEdit(insertAtCursor(ta, `![](${filename})`))
        } catch (err) {
          console.error('Paste image failed:', err)
        }
        return
      }
    }
  }, [currentFilePath, onEdit])

  return (
    <div className="flex-1 overflow-y-auto relative" style={{ display: 'flex', flexDirection: 'column' }}>
      {editing && (
        <MarkdownToolbar textareaRef={textareaRef} onEdit={onEdit} currentFilePath={currentFilePath} />
      )}
      <div style={{ width: '100%', padding: '40px 40px 60px', flex: 1 }}>
        {editing ? (
          <textarea
            ref={textareaRef}
            value={text}
            onChange={e => onEdit(e.target.value)}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            className="w-full min-h-[calc(100vh-160px)] resize-none outline-none border-none text-[15px] leading-[1.75]"
            style={{
              background: 'transparent',
              color: 'var(--text)',
              fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
              caretColor: 'var(--color-accent)',
            }}
            spellCheck={false}
          />
        ) : (
          <article
            className={'md-body theme-' + themeId}
            dangerouslySetInnerHTML={{ __html: html }}
          />
        )}
      </div>
    </div>
  )
}

function hasVisibleContent(html: string): boolean {
  const stripped = html.replace(/<script[\s\S]*?<\/script>/gi, '').replace(/<style[\s\S]*?<\/style>/gi, '')
  const text = stripped.replace(/<[^>]*>/g, '').trim()
  return text.length > 20
}

function HtmlViewer({ html }: { html: string }) {
  const { t } = useI18n()
  const hasContent = useMemo(() => hasVisibleContent(html), [html])
  const [showPreview, setShowPreview] = useState(hasContent)

  const blobUrl = useMemo(() => {
    const blob = new Blob([html], { type: 'text/html;charset=utf-8' })
    return URL.createObjectURL(blob)
  }, [html])

  useEffect(() => {
    return () => URL.revokeObjectURL(blobUrl)
  }, [blobUrl])

  const highlightedHtml = useMemo(() => {
    try {
      const result = hljs.highlight(html, { language: 'xml' })
      return DOMPurify.sanitize(result.value)
    } catch {
      return DOMPurify.sanitize(html.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'))
    }
  }, [html])

  return (
    <div className="flex-1 overflow-hidden relative flex flex-col">
      <div className="flex items-center gap-2 px-4 py-1.5" style={{ borderBottom: '1px solid var(--border-s)', background: 'var(--sidebar-bg)', flexShrink: 0 }}>
        <button
          className="text-[11.5px] px-2.5 py-1 rounded cursor-pointer border-none"
          style={{ background: showPreview ? 'transparent' : 'var(--surface)', color: showPreview ? 'var(--text-3)' : 'var(--text)', fontFamily: 'inherit', fontWeight: showPreview ? 400 : 500, boxShadow: showPreview ? 'none' : 'var(--shadow-sm)' }}
          onClick={() => setShowPreview(false)}
        >
          {t('content.source')}
        </button>
        <button
          className="text-[11.5px] px-2.5 py-1 rounded cursor-pointer border-none"
          style={{ background: showPreview ? 'var(--surface)' : 'transparent', color: showPreview ? 'var(--text)' : 'var(--text-3)', fontFamily: 'inherit', fontWeight: showPreview ? 500 : 400, boxShadow: showPreview ? 'var(--shadow-sm)' : 'none' }}
          onClick={() => setShowPreview(true)}
        >
          {t('content.preview')}
        </button>
      </div>
      {showPreview ? (
        <iframe
          src={blobUrl}
          className="flex-1 w-full border-none"
          sandbox="allow-scripts"
          title="HTML Preview"
        />
      ) : (
        <div className="flex-1 overflow-y-auto">
          <pre
            className="text-[14px] leading-[1.7] p-6 m-0 overflow-x-auto"
            style={{ fontFamily: "'JetBrains Mono', 'Fira Code', monospace", background: 'transparent' }}
          >
            <code className="hljs language-html" dangerouslySetInnerHTML={{ __html: highlightedHtml }} />
          </pre>
        </div>
      )}
    </div>
  )
}
