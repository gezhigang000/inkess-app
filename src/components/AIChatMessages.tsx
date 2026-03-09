import { useState, useRef } from 'react'
import DOMPurify from 'dompurify'
import { readFileLines } from '../lib/tauri'
import { useI18n } from '../lib/i18n'

export interface UIMessage {
  role: 'user' | 'assistant' | 'tool_call' | 'tool_result' | 'system'
  content: string
  toolName?: string
  toolId?: string
}

export interface AIChatMessagesProps {
  messages: UIMessage[]
  streaming: boolean
  config: unknown | null
  searchOpen: boolean
  searchQuery: string
  setSearchOpen: (v: boolean | ((prev: boolean) => boolean)) => void
  setSearchQuery: (v: string) => void
  onOpenFile?: (path: string, line?: number) => void
  onShowConfig: () => void
  currentDir: string
  messagesEndRef: React.RefObject<HTMLDivElement | null>
}

/** Highlight search query matches in text with <mark> tags */
function highlightText(text: string, query: string): React.ReactNode {
  if (!query) return text
  const lc = text.toLowerCase()
  const lcq = query.toLowerCase()
  const parts: React.ReactNode[] = []
  let last = 0
  let idx = lc.indexOf(lcq, last)
  while (idx !== -1) {
    if (idx > last) parts.push(text.slice(last, idx))
    parts.push(<mark key={idx}>{text.slice(idx, idx + query.length)}</mark>)
    last = idx + query.length
    idx = lc.indexOf(lcq, last)
  }
  if (last < text.length) parts.push(text.slice(last))
  return parts.length > 0 ? <>{parts}</> : text
}

/** Collapsible text block for tool call/result — collapses long content */
function CollapsibleBlock({ text, maxLines = 3, label }: { text: string; maxLines?: number; label?: string }) {
  const [expanded, setExpanded] = useState(false)
  const lines = text.split('\n')
  const needsCollapse = lines.length > maxLines + 2
  const displayText = needsCollapse && !expanded ? lines.slice(0, maxLines).join('\n') : text

  return (
    <div>
      {label && <div style={{ fontSize: 10, color: 'var(--text-3)', marginBottom: 2 }}>{label}</div>}
      <pre style={{ fontSize: 11, color: 'var(--text-2)', whiteSpace: 'pre-wrap', wordBreak: 'break-all', margin: 0 }}>
        {displayText}
        {needsCollapse && !expanded && '\n...'}
      </pre>
      {needsCollapse && (
        <button
          onClick={() => setExpanded(v => !v)}
          style={{ fontSize: 10, color: 'var(--accent)', background: 'none', border: 'none', cursor: 'pointer', padding: '2px 0', marginTop: 2 }}
        >
          {expanded ? '▲ Collapse' : `▼ Show all (${lines.length} lines)`}
        </button>
      )}
    </div>
  )
}

/** Specialized block for run_python tool calls — collapsed by default with code styling */
function PythonCodeBlock({ content }: { content: string }) {
  const [expanded, setExpanded] = useState(false)

  let code = content
  try {
    const parsed = JSON.parse(content)
    if (parsed.code) code = parsed.code
  } catch { /* use raw content */ }

  const lines = code.split('\n')
  // Extract summary from first comment line
  const commentLine = lines.find(l => /^\s*#[^!]/.test(l))
  const summary = commentLine
    ? commentLine.trim().replace(/^#+\s*/, '')
    : lines.find(l => l.trim() && !l.trim().startsWith('import') && !l.trim().startsWith('from') && !l.trim().startsWith('#'))?.trim().slice(0, 60)
      || 'Python code'

  return (
    <div>
      <button
        onClick={() => setExpanded(v => !v)}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, width: '100%',
          fontSize: 11, color: 'var(--text-2)', background: 'none', border: 'none',
          cursor: 'pointer', padding: '2px 0', textAlign: 'left',
        }}
      >
        <span style={{ flexShrink: 0 }}>{expanded ? '▼' : '▶'}</span>
        <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {summary}
        </span>
        <span style={{ flexShrink: 0, fontSize: 10, color: 'var(--text-3)' }}>
          {lines.length} lines
        </span>
      </button>
      {expanded && (
        <pre className="ai-code-block" style={{ marginTop: 4 }}>
          <code>{code}</code>
        </pre>
      )}
    </div>
  )
}

// Simple markdown-like rendering for AI messages
function MessageContent({ text, onOpenFile, currentDir, hoverTimerRef, setHoverPreview }: {
  text: string
  onOpenFile?: (path: string, line?: number) => void
  currentDir?: string
  hoverTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
  setHoverPreview: (v: { x: number; y: number; content: string; path: string; line: number } | null) => void
}) {
  // Split by code blocks
  const parts = text.split(/(```[\s\S]*?```)/g)
  return (
    <>
      {parts.map((part, i) => {
        if (part.startsWith('```') && part.endsWith('```')) {
          const inner = part.slice(3, -3)
          const newline = inner.indexOf('\n')
          const code = newline >= 0 ? inner.slice(newline + 1) : inner
          return (
            <pre key={i} className="ai-code-block">
              <code>{code}</code>
            </pre>
          )
        }
        // Bold + inline code
        const html = part
          .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
          .replace(/`([^`]+)`/g, '<code class="ai-inline-code">$1</code>')
          // File references: path/to/file.ext:42 or C:\path\file.ext:42-50
          .replace(
            /(?<![`<\w])(?:[a-zA-Z]:[/\\])?([a-zA-Z0-9_./\\-]+\.[a-zA-Z0-9]+):(\d+)(?:-(\d+))?(?![`>\w])/g,
            (_match, filePath, lineStart, _lineEnd) => {
              const line = parseInt(lineStart, 10)
              return `<a class="ai-file-link" data-path="${DOMPurify.sanitize(filePath)}" data-line="${line}" title="${filePath}:${lineStart}${_lineEnd ? '-' + _lineEnd : ''}">${filePath}:${lineStart}${_lineEnd ? '-' + _lineEnd : ''}</a>`
            }
          )
        return (
          <span
            key={i}
            dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(html, { ADD_ATTR: ['data-path', 'data-line'] }) }}
            onClick={(e) => {
              const target = e.target as HTMLElement
              if (target.classList.contains('ai-file-link')) {
                const path = target.getAttribute('data-path')
                const line = parseInt(target.getAttribute('data-line') || '0', 10)
                if (path && onOpenFile) onOpenFile(path, line || undefined)
              }
            }}
            onMouseOver={(e) => {
              const target = e.target as HTMLElement
              if (target.classList.contains('ai-file-link')) {
                const path = target.getAttribute('data-path')
                const line = parseInt(target.getAttribute('data-line') || '0', 10)
                if (path && line > 0) {
                  if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
                  hoverTimerRef.current = setTimeout(async () => {
                    try {
                      const fullPath = currentDir ? `${currentDir}/${path}` : path
                      const content = await readFileLines(fullPath, line, 3)
                      const rect = target.getBoundingClientRect()
                      setHoverPreview({ x: rect.left, y: rect.bottom + 4, content, path, line })
                    } catch { /* skip */ }
                  }, 200)
                }
              }
            }}
            onMouseOut={(e) => {
              const target = e.target as HTMLElement
              if (target.classList.contains('ai-file-link')) {
                if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
                setHoverPreview(null)
              }
            }}
          />
        )
      })}
    </>
  )
}

export function AIChatMessages({
  messages, streaming, config, searchOpen, searchQuery,
  setSearchOpen, setSearchQuery, onOpenFile, onShowConfig,
  currentDir, messagesEndRef,
}: AIChatMessagesProps) {
  const { t } = useI18n()
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [hoverPreview, setHoverPreview] = useState<{ x: number; y: number; content: string; path: string; line: number } | null>(null)

  return (
    <>
      {/* Search bar */}
      {searchOpen && (
        <div className="ai-search-bar">
          <input
            autoFocus
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            placeholder={t('ai.searchMessages')}
            onKeyDown={e => { if (e.key === 'Escape') { setSearchOpen(false); setSearchQuery('') } }}
          />
          <span className="ai-search-count">
            {searchQuery ? messages.filter(m => m.content.toLowerCase().includes(searchQuery.toLowerCase())).length : 0} {t('ai.matches')}
          </span>
          <button onClick={() => { setSearchOpen(false); setSearchQuery('') }} aria-label={t('ai.close')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 12, height: 12 }}>
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      )}
      {/* Messages */}
      <div className="ai-messages" aria-live="polite">
        {messages.length === 0 && (
          <div style={{ textAlign: 'center', padding: '40px 20px', color: 'var(--text-3)', fontSize: 13 }}>
            {config ? t('ai.startChat') : (
              <button className="git-btn git-btn-primary" style={{ fontSize: 12 }} onClick={onShowConfig}>
                {t('ai.configModel')}
              </button>
            )}
          </div>
        )}
        {messages.filter(msg => !searchQuery || msg.content.toLowerCase().includes(searchQuery.toLowerCase())).map((msg, i) => {
          // Format MCP tool names: mcp__{serverid}__{toolname} → serverid / toolname
          const displayToolName = msg.toolName?.startsWith('mcp__')
            ? msg.toolName.slice(5).replace('__', ' / ')
            : msg.toolName
          return (
          <div key={i} className={`ai-msg ai-msg-${msg.role}`}>
            {msg.role === 'tool_call' ? (
              <div className="ai-msg-tool">
                <div style={{ fontSize: 11, color: 'var(--text-3)', marginBottom: 2 }}>
                  {t('ai.toolCall')} <span style={{ fontWeight: 600 }}>{displayToolName}</span>
                </div>
                {msg.toolName === 'run_python' ? (
                  <PythonCodeBlock content={msg.content} />
                ) : (
                  <CollapsibleBlock text={msg.content} maxLines={3} />
                )}
              </div>
            ) : msg.role === 'tool_result' ? (
              <div className="ai-msg-tool">
                <div style={{ fontSize: 11, color: 'var(--text-3)', marginBottom: 2 }}>
                  {t('ai.toolResult', { name: displayToolName || '' })}
                </div>
                <CollapsibleBlock text={msg.content.length > 2000 ? msg.content.slice(0, 2000) + '...' : msg.content} maxLines={5} />
                {msg.toolName === 'write_file' && msg.content.startsWith('File written:') && (() => {
                  const match = msg.content.match(/^File written: (.+?) \(/)
                  const filePath = match?.[1]
                  if (!filePath) return null
                  const ext = filePath.split('.').pop()?.toLowerCase()
                  const viewable = ['html', 'htm', 'md', 'txt', 'json', 'csv'].includes(ext || '')
                  if (!viewable) return null
                  return (
                    <button
                      className="git-btn"
                      style={{ fontSize: 10, marginTop: 4, padding: '2px 8px' }}
                      onClick={() => onOpenFile?.(filePath)}
                    >
                      {t('ai.openPreview')}
                    </button>
                  )
                })()}
              </div>
            ) : msg.role === 'system' ? (
              <div style={{ textAlign: 'center', fontSize: 11, color: 'var(--text-3)', padding: '4px 0' }}>
                {msg.content}
              </div>
            ) : (
              <div className={msg.role === 'user' ? 'ai-bubble ai-bubble-user' : 'ai-bubble ai-bubble-assistant'}>
                {msg.role === 'user' && searchQuery ? (
                  <div style={{ whiteSpace: 'pre-wrap' }}>{highlightText(msg.content, searchQuery)}</div>
                ) : (
                  <MessageContent text={msg.content} onOpenFile={onOpenFile} currentDir={currentDir} hoverTimerRef={hoverTimerRef} setHoverPreview={setHoverPreview} />
                )}
              </div>
            )}
          </div>
          )
        })}
        {streaming && messages[messages.length - 1]?.role !== 'assistant' && (
          <div className="ai-msg ai-msg-assistant">
            <div className="ai-bubble ai-bubble-assistant">
              <span className="ai-typing-indicator" />
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Hover preview tooltip */}
      {hoverPreview && (
        <div
          className="ai-code-tooltip"
          style={{
            position: 'fixed',
            left: Math.min(hoverPreview.x, window.innerWidth - 420),
            top: Math.min(hoverPreview.y, window.innerHeight - 220),
          }}
          onMouseEnter={() => { /* keep visible */ }}
          onMouseLeave={() => setHoverPreview(null)}
        >
          <div style={{ fontSize: 10, color: 'var(--text-3)', marginBottom: 4 }}>{hoverPreview.path}:{hoverPreview.line}</div>
          <pre style={{ margin: 0, fontSize: 11, lineHeight: 1.5, whiteSpace: 'pre', overflow: 'auto' }}>{hoverPreview.content}</pre>
        </div>
      )}
    </>
  )
}
