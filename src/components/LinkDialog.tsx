import { useState, useRef, useEffect, useCallback } from 'react'
import { useI18n } from '../lib/i18n'

interface LinkDialogProps {
  initialText?: string
  onInsert: (url: string, text: string) => void
  onClose: () => void
}

export function LinkDialog({ initialText, onInsert, onClose }: LinkDialogProps) {
  const { t } = useI18n()
  const [url, setUrl] = useState('')
  const [text, setText] = useState(initialText || '')
  const ref = useRef<HTMLDivElement>(null)
  const urlRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    urlRef.current?.focus()
  }, [])

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose()
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [onClose])

  const handleSubmit = useCallback(() => {
    const trimmedUrl = url.trim()
    if (!trimmedUrl) return
    // Block dangerous protocols (javascript:, data:, vbscript:)
    const lower = trimmedUrl.toLowerCase()
    if (lower.startsWith('javascript:') || lower.startsWith('data:') || lower.startsWith('vbscript:')) return
    onInsert(trimmedUrl, text.trim() || trimmedUrl)
  }, [url, text, onInsert])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      handleSubmit()
    }
    if (e.key === 'Escape') onClose()
  }, [handleSubmit, onClose])

  return (
    <div ref={ref} className="link-dialog" onKeyDown={handleKeyDown}>
      <input
        ref={urlRef}
        type="text"
        placeholder={t('mdToolbar.linkUrl')}
        value={url}
        onChange={e => setUrl(e.target.value)}
      />
      <input
        type="text"
        placeholder={t('mdToolbar.linkText')}
        value={text}
        onChange={e => setText(e.target.value)}
      />
      <div className="link-dialog-actions">
        <button onClick={handleSubmit}>{t('mdToolbar.insert')}</button>
      </div>
    </div>
  )
}
