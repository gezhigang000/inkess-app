import DOMPurify from 'dompurify'

interface DocxViewerProps {
  html: string
}

export function DocxViewer({ html }: DocxViewerProps) {
  const sanitized = DOMPurify.sanitize(html)

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-[860px] mx-auto w-full px-20 py-10">
        <div
          className="docx-preview text-[15px] leading-[1.75]"
          style={{ color: 'var(--text)' }}
          dangerouslySetInnerHTML={{ __html: sanitized }}
        />
      </div>
    </div>
  )
}
