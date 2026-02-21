import { useState } from 'react'
import { useI18n } from '../lib/i18n'

interface NewFilePopupProps {
  type: 'file' | 'folder'
  onConfirm: (name: string, template?: string) => void
  onCancel: () => void
}

const FILE_TEMPLATES: { label: string; ext: string; content: string }[] = [
  { label: 'Empty', ext: '.txt', content: '' },
  { label: 'Markdown', ext: '.md', content: '# New Document\n\n' },
  { label: 'JSON', ext: '.json', content: '{\n  \n}\n' },
  { label: 'HTML', ext: '.html', content: '<!DOCTYPE html>\n<html>\n<head><title></title></head>\n<body>\n\n</body>\n</html>\n' },
  { label: 'CSS', ext: '.css', content: '' },
  { label: 'JavaScript', ext: '.js', content: '' },
  { label: 'TypeScript', ext: '.ts', content: '' },
]

export function NewFilePopup({ type, onConfirm, onCancel }: NewFilePopupProps) {
  const { t } = useI18n()
  const [name, setName] = useState('')
  const [templateIdx, setTemplateIdx] = useState(1) // default Markdown

  const handleSubmit = () => {
    const trimmed = name.trim()
    if (!trimmed) return
    if (type === 'folder') {
      onConfirm(trimmed)
    } else {
      const tpl = FILE_TEMPLATES[templateIdx]
      const finalName = trimmed.includes('.') ? trimmed : trimmed + tpl.ext
      onConfirm(finalName, tpl.content)
    }
  }

  return (
    <div className="shortcuts-backdrop" onClick={onCancel}>
      <div className="shortcuts-modal" onClick={e => e.stopPropagation()}
        style={{ minWidth: 320 }}
      >
        <div className="flex items-center justify-between mb-1">
          <h3 style={{ margin: 0 }}>{type === 'file' ? t('newFile.title') : t('newFolder.title')}</h3>
          <button className="sidebar-action-btn" onClick={onCancel} aria-label={t('ai.close')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
        {type === 'file' && (
          <div className="flex flex-wrap gap-2 mb-5">
            {FILE_TEMPLATES.map((tpl, i) => (
              <button
                key={tpl.label}
                className="new-file-tpl-btn"
                style={{
                  background: i === templateIdx ? 'var(--color-accent)' : 'var(--sidebar-bg)',
                  color: i === templateIdx ? '#fff' : 'var(--text-2)',
                }}
                onClick={() => setTemplateIdx(i)}
              >
                {tpl.label === 'Empty' ? t('newFile.emptyFile') : tpl.label}
              </button>
            ))}
          </div>
        )}
        <input
          autoFocus
          className="new-file-input"
          placeholder={type === 'file' ? t('newFile.placeholder.file') : t('newFile.placeholder.folder')}
          value={name}
          onChange={e => setName(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') handleSubmit(); if (e.key === 'Escape') onCancel() }}
        />
        <div className="flex justify-end gap-3 mt-6">
          <button className="toolbar-btn" onClick={onCancel}>{t('newFile.cancel')}</button>
          <button className="toolbar-btn toolbar-btn-accent" onClick={handleSubmit}>{t('newFile.create')}</button>
        </div>
      </div>
    </div>
  )
}
