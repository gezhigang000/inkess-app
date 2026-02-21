import { useState, useCallback, useRef } from 'react'
import { useI18n } from '../lib/i18n'
import {
  toggleInlineWrap,
  toggleLinePrefix,
  insertCodeBlock,
  insertHorizontalRule,
  insertAtCursor,
  generateTable,
  getSelection,
} from '../lib/markdownFormat'
import { copyFileToDir, openImageDialog, saveFile } from '../lib/tauri'
import { TableBuilderDialog } from './TableBuilderDialog'
import { LinkDialog } from './LinkDialog'

interface MarkdownToolbarProps {
  textareaRef: React.RefObject<HTMLTextAreaElement | null>
  onEdit: (text: string) => void
  currentFilePath?: string
}

export function MarkdownToolbar({ textareaRef, onEdit, currentFilePath }: MarkdownToolbarProps) {
  const { t } = useI18n()
  const [headingOpen, setHeadingOpen] = useState(false)
  const [tableOpen, setTableOpen] = useState(false)
  const [linkOpen, setLinkOpen] = useState(false)
  const headingRef = useRef<HTMLDivElement>(null)
  const tableRef = useRef<HTMLDivElement>(null)
  const linkRef = useRef<HTMLDivElement>(null)

  const withTa = useCallback((fn: (ta: HTMLTextAreaElement) => string) => {
    const ta = textareaRef.current
    if (!ta) return
    const newText = fn(ta)
    onEdit(newText)
  }, [textareaRef, onEdit])

  const handleBold = useCallback(() => withTa(ta => toggleInlineWrap(ta, '**')), [withTa])
  const handleItalic = useCallback(() => withTa(ta => toggleInlineWrap(ta, '*')), [withTa])
  const handleStrike = useCallback(() => withTa(ta => toggleInlineWrap(ta, '~~')), [withTa])
  const handleCode = useCallback(() => withTa(ta => toggleInlineWrap(ta, '`')), [withTa])
  const handleUl = useCallback(() => withTa(ta => toggleLinePrefix(ta, '- ')), [withTa])
  const handleOl = useCallback(() => withTa(ta => toggleLinePrefix(ta, '1. ')), [withTa])
  const handleQuote = useCallback(() => withTa(ta => toggleLinePrefix(ta, '> ')), [withTa])
  const handleCodeBlock = useCallback(() => withTa(ta => insertCodeBlock(ta)), [withTa])
  const handleHr = useCallback(() => withTa(ta => insertHorizontalRule(ta)), [withTa])

  const handleHeading = useCallback((level: number) => {
    withTa(ta => toggleLinePrefix(ta, '#'.repeat(level) + ' '))
    setHeadingOpen(false)
  }, [withTa])

  const handleTableInsert = useCallback((rows: number, cols: number) => {
    withTa(ta => insertAtCursor(ta, generateTable(rows, cols)))
    setTableOpen(false)
  }, [withTa])

  const handleLinkInsert = useCallback((url: string, text: string) => {
    withTa(ta => insertAtCursor(ta, `[${text}](${url})`))
    setLinkOpen(false)
  }, [withTa])

  const handleImage = useCallback(async () => {
    if (!currentFilePath) {
      alert(t('mdToolbar.saveFirst'))
      return
    }
    const selected = await openImageDialog()
    if (!selected) return
    // Get parent directory of current markdown file
    const parts = currentFilePath.replace(/\\/g, '/').split('/')
    parts.pop()
    const parentDir = parts.join('/')
    try {
      const filename = await copyFileToDir(selected, parentDir)
      withTa(ta => insertAtCursor(ta, `![](${filename})`))
    } catch (e) {
      console.error('Image insert failed:', e)
    }
  }, [currentFilePath, withTa, t])

  const getLinkInitialText = useCallback(() => {
    const ta = textareaRef.current
    if (!ta) return ''
    return getSelection(ta).text
  }, [textareaRef])

  return (
    <div className="md-toolbar">
      {/* Text formatting */}
      <div className="md-toolbar-group">
        <button className="md-toolbar-btn" title={t('mdToolbar.bold')} onClick={handleBold}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M6 4h8a4 4 0 0 1 0 8H6zM6 12h9a4 4 0 0 1 0 8H6z"/></svg>
        </button>
        <button className="md-toolbar-btn" title={t('mdToolbar.italic')} onClick={handleItalic}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="19" y1="4" x2="10" y2="4"/><line x1="14" y1="20" x2="5" y2="20"/><line x1="15" y1="4" x2="9" y2="20"/></svg>
        </button>
        <button className="md-toolbar-btn" title={t('mdToolbar.strikethrough')} onClick={handleStrike}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="4" y1="12" x2="20" y2="12"/><path d="M17.5 7.5c0-2-1.5-3.5-5.5-3.5S6.5 5.5 6.5 7.5c0 4 11 4 11 8.5 0 2-2 3.5-5.5 3.5s-5.5-1.5-5.5-3.5"/></svg>
        </button>
        <button className="md-toolbar-btn" title={t('mdToolbar.code')} onClick={handleCode}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>
        </button>
      </div>

      <div className="md-toolbar-sep" />

      {/* Headings */}
      <div className="md-toolbar-group" style={{ position: 'relative' }} ref={headingRef}>
        <button className="md-toolbar-btn" title={t('mdToolbar.heading')} onClick={() => setHeadingOpen(v => !v)} style={{ width: 'auto', padding: '0 6px', fontSize: 12, fontWeight: 600 }}>
          H
          <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 8, height: 8, marginLeft: 2 }}><polyline points="2 4 6 8 10 4"/></svg>
        </button>
        {headingOpen && (
          <div className="md-toolbar-dropdown">
            {[1, 2, 3].map(level => (
              <button key={level} onClick={() => handleHeading(level)} style={{ fontSize: 16 - level * 1.5, fontWeight: 600 }}>
                {'#'.repeat(level)} Heading {level}
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="md-toolbar-sep" />

      {/* Lists & blocks */}
      <div className="md-toolbar-group">
        <button className="md-toolbar-btn" title={t('mdToolbar.ul')} onClick={handleUl}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="9" y1="6" x2="20" y2="6"/><line x1="9" y1="12" x2="20" y2="12"/><line x1="9" y1="18" x2="20" y2="18"/><circle cx="5" cy="6" r="1" fill="currentColor"/><circle cx="5" cy="12" r="1" fill="currentColor"/><circle cx="5" cy="18" r="1" fill="currentColor"/></svg>
        </button>
        <button className="md-toolbar-btn" title={t('mdToolbar.ol')} onClick={handleOl}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="10" y1="6" x2="20" y2="6"/><line x1="10" y1="12" x2="20" y2="12"/><line x1="10" y1="18" x2="20" y2="18"/><text x="4" y="8" fontSize="7" fill="currentColor" stroke="none" fontFamily="inherit">1</text><text x="4" y="14" fontSize="7" fill="currentColor" stroke="none" fontFamily="inherit">2</text><text x="4" y="20" fontSize="7" fill="currentColor" stroke="none" fontFamily="inherit">3</text></svg>
        </button>
        <button className="md-toolbar-btn" title={t('mdToolbar.quote')} onClick={handleQuote}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z"/><path d="M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z"/></svg>
        </button>
      </div>

      <div className="md-toolbar-sep" />

      {/* Block-level */}
      <div className="md-toolbar-group">
        <button className="md-toolbar-btn" title={t('mdToolbar.codeBlock')} onClick={handleCodeBlock}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="3" width="18" height="18" rx="2"/><polyline points="9 8 5 12 9 16"/><polyline points="15 8 19 12 15 16"/></svg>
        </button>
        <button className="md-toolbar-btn" title={t('mdToolbar.hr')} onClick={handleHr}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="3" y1="12" x2="21" y2="12"/></svg>
        </button>
      </div>

      <div className="md-toolbar-sep" />

      {/* Insert */}
      <div className="md-toolbar-group">
        <div style={{ position: 'relative' }} ref={linkRef}>
          <button className="md-toolbar-btn" title={t('mdToolbar.link')} onClick={() => setLinkOpen(v => !v)}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>
          </button>
          {linkOpen && (
            <LinkDialog
              initialText={getLinkInitialText()}
              onInsert={handleLinkInsert}
              onClose={() => setLinkOpen(false)}
            />
          )}
        </div>
        <button className="md-toolbar-btn" title={t('mdToolbar.image')} onClick={handleImage}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>
        </button>
        <div style={{ position: 'relative' }} ref={tableRef}>
          <button className="md-toolbar-btn" title={t('mdToolbar.table')} onClick={() => setTableOpen(v => !v)}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="3" y1="9" x2="21" y2="9"/><line x1="3" y1="15" x2="21" y2="15"/><line x1="9" y1="3" x2="9" y2="21"/><line x1="15" y1="3" x2="15" y2="21"/></svg>
          </button>
          {tableOpen && (
            <TableBuilderDialog
              onInsert={handleTableInsert}
              onClose={() => setTableOpen(false)}
            />
          )}
        </div>
      </div>
    </div>
  )
}
