import { useRef, useEffect, useMemo } from 'react'
import hljs from 'highlight.js'
import DOMPurify from 'dompurify'

interface CodeViewerProps {
  text: string
  language: string
  editing: boolean
  onEdit: (text: string) => void
}

export function CodeViewer({ text, language, editing, onEdit }: CodeViewerProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (editing && textareaRef.current) textareaRef.current.focus()
  }, [editing])

  const highlightedHtml = useMemo(() => {
    try {
      const result = hljs.getLanguage(language)
        ? hljs.highlight(text, { language })
        : hljs.highlightAuto(text)
      return DOMPurify.sanitize(result.value)
    } catch {
      return DOMPurify.sanitize(text)
    }
  }, [text, language])

  const lineCount = useMemo(() => text.split('\n').length, [text])

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="w-full px-4 py-4">
        {editing ? (
          <textarea
            ref={textareaRef}
            value={text}
            onChange={e => onEdit(e.target.value)}
            className="w-full min-h-[calc(100vh-160px)] resize-none outline-none border-none text-[13.5px] leading-[1.6]"
            style={{
              background: 'transparent',
              color: 'var(--text)',
              fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
              caretColor: 'var(--accent)',
              tabSize: 4,
            }}
            spellCheck={false}
          />
        ) : (
          <div>
            {/* Language badge */}
            <div style={{
              display: 'flex',
              justifyContent: 'flex-end',
              marginBottom: 6,
            }}>
              <span style={{
                fontSize: 10,
                fontWeight: 600,
                fontFamily: "'JetBrains Mono', monospace",
                padding: '2px 8px',
                borderRadius: 4,
                background: 'var(--sidebar-bg)',
                color: 'var(--text-3)',
                textTransform: 'uppercase',
                letterSpacing: '0.04em',
                userSelect: 'none',
              }}>
                {language}
              </span>
            </div>
            <pre
              className="overflow-x-auto"
              style={{
                background: 'var(--ink-900)',
                borderRadius: 'var(--radius)',
                padding: '20px 0',
                margin: 0,
                fontSize: '13.5px',
                lineHeight: 1.6,
                fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
                display: 'flex',
              }}
            >
              {/* Line numbers */}
              <div
                aria-hidden="true"
                style={{
                  padding: '0 0 0 20px',
                  minWidth: 48,
                  textAlign: 'right',
                  color: 'rgba(255,255,255,0.18)',
                  userSelect: 'none',
                  flexShrink: 0,
                  fontFamily: 'inherit',
                  fontSize: 'inherit',
                  lineHeight: 'inherit',
                }}
              >
                {Array.from({ length: lineCount }, (_, i) => (
                  <div key={i}>{i + 1}</div>
                ))}
              </div>
              {/* Code content */}
              <code
                className={`hljs language-${language}`}
                style={{
                  padding: '0 24px 0 16px',
                  flex: 1,
                  minWidth: 0,
                }}
                dangerouslySetInnerHTML={{ __html: highlightedHtml }}
              />
            </pre>
          </div>
        )}
      </div>
    </div>
  )
}
