import { useRef, useEffect } from 'react'

interface TextViewerProps {
  text: string
  editing: boolean
  onEdit: (text: string) => void
}

export function TextViewer({ text, editing, onEdit }: TextViewerProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (editing && textareaRef.current) textareaRef.current.focus()
  }, [editing])

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-[860px] mx-auto w-full px-20 py-10">
        {editing ? (
          <textarea
            ref={textareaRef}
            value={text}
            onChange={e => onEdit(e.target.value)}
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
          <pre
            className="text-[15px] leading-[1.75] whitespace-pre-wrap break-words"
            style={{ color: 'var(--text)', fontFamily: "'JetBrains Mono', 'Fira Code', monospace" }}
          >
            {text}
          </pre>
        )}
      </div>
    </div>
  )
}
