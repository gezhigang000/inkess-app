import { useState, useEffect, useRef } from 'react'
import { type ThemeId } from '../lib/themes'
import { useI18n } from '../lib/i18n'
import { SearchBar } from './SearchBar'

interface ToolbarProps {
  themeId: ThemeId
  onToggleSidebar: () => void
  currentFile: string
  currentDir: string
  currentFilePath: string
  onExport: (format: string) => void
  onNavigateDir: (path: string) => void
  isViewingHistory: boolean
  onBackToLatest: () => void
  isEditing: boolean
  viewMode?: 'preview' | 'edit' | 'split'
  onToggleEdit: () => void
  hasUnsavedChanges: boolean
  onOpenFile: () => void
  onOpenFilePath: (path: string) => void
  isReadOnly: boolean
  loading: boolean
  devMode: boolean
  onToggleDevMode: () => void
  onToggleAI?: () => void
  aiPanelOpen?: boolean
  onOpenSettings?: () => void
  isPro?: boolean
  onOpenLicense?: () => void
}

export function Toolbar({
  themeId, onToggleSidebar,
  currentFile, currentDir, currentFilePath, onExport, onNavigateDir,
  isViewingHistory, onBackToLatest, isEditing, viewMode, onToggleEdit,
  hasUnsavedChanges, onOpenFile, onOpenFilePath, isReadOnly, loading, devMode, onToggleDevMode,
  onToggleAI, aiPanelOpen, onOpenSettings, isPro, onOpenLicense,
}: ToolbarProps) {
  const { t } = useI18n()
  const [exportOpen, setExportOpen] = useState(false)
  const exportRef = useRef<HTMLDivElement>(null)

  const dirParts = currentDir ? currentDir.split('/').filter(Boolean) : []
  const breadcrumbParts = dirParts.slice(-3)
  const breadcrumbOffset = dirParts.length - breadcrumbParts.length

  useEffect(() => {
    if (!exportOpen) return
    const handler = (e: MouseEvent) => {
      if (exportRef.current && !exportRef.current.contains(e.target as Node)) {
        setExportOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [exportOpen])

  const noDir = !currentDir
  const disabledTitle = noDir ? t('toolbar.needWorkspace') : undefined

  return (
    <>
    <header
      role="toolbar"
      aria-label={t('toolbar.toggleSidebar')}
      className="fixed top-0 left-0 right-0 h-[52px] z-50 flex items-center gap-2"
      style={{
        background: 'var(--toolbar-bg)',
        backdropFilter: 'blur(20px) saturate(1.8)',
        WebkitBackdropFilter: 'blur(20px) saturate(1.8)',
        borderBottom: '1px solid var(--border-s)',
        padding: '0 16px',
      }}
    >
      <div className="flex items-center gap-1.5 mr-1 select-none">
        <svg viewBox="0 0 108 108" className="w-5 h-5 rounded-[4px]" style={{ flexShrink: 0 }}>
          <rect width="108" height="108" fill="var(--text)" rx="22" ry="22"/>
          <path d="M54 18 C54 18, 36 46, 36 60 C36 70.5 44.06 79 54 79 C63.94 79 72 70.5 72 60 C72 46 54 18 54 18Z" fill="var(--bg, #fff)"/>
          <ellipse cx="54" cy="84" rx="14" ry="3.5" fill="none" stroke="var(--bg, #fff)" strokeWidth="1.8" opacity="0.35"/>
          <ellipse cx="54" cy="84" rx="8" ry="2" fill="none" stroke="var(--bg, #fff)" strokeWidth="1.2" opacity="0.2"/>
        </svg>
        <span className="font-semibold text-[17px] tracking-tight toolbar-logo-text" style={{ fontFamily: "'Crimson Pro', serif", letterSpacing: '-0.02em' }}>
          Ink<span className="text-accent">ess</span>
        </span>
      </div>
      <div className="w-px h-5 mx-2" style={{ background: 'var(--border)' }} />
      <div className="flex items-center gap-0.5 text-[13px]" style={{ color: 'var(--text-2)' }}>
        {breadcrumbParts.map((part, i) => {
          const fullPath = '/' + dirParts.slice(0, breadcrumbOffset + i + 1).join('/')
          return (
            <span key={i} className="flex items-center gap-0.5">
              <span
                className="cursor-pointer transition-colors toolbar-breadcrumb"
                onClick={() => onNavigateDir(fullPath)}
              >
                {part}
              </span>
              <span className="mx-0.5 opacity-40">/</span>
            </span>
          )
        })}
        <span
          className="font-medium"
          style={{ color: 'var(--text)' }}
          title={currentFilePath || undefined}
        >
          {currentFile}
          {hasUnsavedChanges && <span style={{ color: 'var(--color-accent)' }}> *</span>}
        </span>
        {isReadOnly && (
          <span className="ml-1.5 text-[10px] px-1.5 py-0.5 rounded" style={{ background: 'var(--sidebar-bg)', color: 'var(--text-3)' }}>
            {t('toolbar.readOnly')}
          </span>
        )}
        {isViewingHistory && (
          <button
            className="ml-2 px-2 py-0.5 rounded text-[11px] font-medium cursor-pointer border-none toolbar-badge"
            style={{ background: 'var(--color-accent)', color: '#fff' }}
            onClick={onBackToLatest}
          >
            {t('toolbar.backToLatest')}
          </button>
        )}
      </div>
      <div className="flex-1" />

      {loading && (
        <div className="loading-spinner" style={{ width: 16, height: 16, borderWidth: 2 }} />
      )}

      <Btn onClick={onToggleSidebar} title={t('toolbar.toggleSidebar')} aria-label={t('toolbar.toggleSidebar')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
          <rect x="3" y="3" width="18" height="18" rx="2" /><line x1="9" y1="3" x2="9" y2="21" />
        </svg>
      </Btn>

      <Btn onClick={onOpenFile} title={t('toolbar.open')} aria-label={t('toolbar.open.label')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
          <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
        </svg>
        <span className="btn-label">{t('toolbar.open.label')}</span>
      </Btn>

      <SearchBar currentDir={currentDir} onOpenFile={onOpenFilePath} onOpenDir={onNavigateDir} />

      <Btn onClick={onToggleEdit} active={isEditing} disabled={noDir} title={noDir ? disabledTitle : (viewMode === 'preview' ? t('toolbar.editMode') : viewMode === 'edit' ? t('toolbar.splitView') : t('toolbar.previewMode'))} aria-label={t('toolbar.editMode.label')}>
        {viewMode === 'split' ? (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
            <rect x="3" y="3" width="18" height="18" rx="2" /><line x1="12" y1="3" x2="12" y2="21" />
          </svg>
        ) : (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
            <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7" />
            <path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z" />
          </svg>
        )}
        <span className="btn-label">{viewMode === 'preview' ? t('toolbar.edit') : viewMode === 'edit' ? t('toolbar.split') : t('toolbar.read')}</span>
      </Btn>

      <div className="relative" ref={exportRef}>
        <Btn accent onClick={() => setExportOpen(v => !v)} disabled={noDir} title={noDir ? disabledTitle : t('toolbar.export')} aria-label={t('toolbar.export')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
            <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
            <polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" />
          </svg>
          <span className="btn-label">{t('toolbar.export')}</span>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"
            className="w-3 h-3 -ml-0.5 transition-transform"
            style={{ transform: exportOpen ? 'rotate(180deg)' : 'none' }}
          >
            <polyline points="6 9 12 15 18 9" />
          </svg>
        </Btn>
        {exportOpen && (
          <div
            className="absolute top-[calc(100%+6px)] right-0 min-w-[200px] rounded-[10px] p-1.5 z-50"
            style={{
              background: 'var(--surface)',
              border: '1px solid var(--border)',
              boxShadow: 'var(--shadow-lg)',
            }}
          >
            {['PDF', 'DOCX', 'HTML', 'PPTX'].map(fmt => (
              <button
                key={fmt}
                className="toolbar-menu-item flex items-center gap-2.5 w-full px-3 py-2 rounded-md text-[13px] text-left transition-colors cursor-pointer"
                style={{ background: 'none', color: 'var(--text)', fontFamily: 'inherit', border: 'none' }}
                onClick={() => { setExportOpen(false); onExport(fmt) }}
              >
                <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded" style={{ background: 'var(--sidebar-bg)', color: 'var(--text-3)', fontFamily: "'JetBrains Mono', monospace" }}>
                  {fmt}
                </span>
                {t('toolbar.exportAs', { fmt: fmt === 'DOCX' ? 'Word' : fmt === 'PPTX' ? 'PPT' : fmt })}
              </button>
            ))}
          </div>
        )}
      </div>

      {onToggleAI && (
        <Btn onClick={onToggleAI} active={aiPanelOpen} disabled={noDir} title={noDir ? disabledTitle : t('toolbar.ai')} aria-label={t('toolbar.ai')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
            <path d="M12 2a2 2 0 012 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 017 7h1a1 1 0 110 2h-1.07A7 7 0 0113 22h-2a7 7 0 01-6.93-6H3a1 1 0 110-2h1a7 7 0 017-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 012-2z" />
          </svg>
          <span className="btn-label">AI</span>
        </Btn>
      )}

      <Btn onClick={onToggleDevMode} active={devMode} disabled={noDir} title={noDir ? disabledTitle : t('toolbar.devMode')} aria-label={t('toolbar.devMode')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
          <polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" />
        </svg>
      </Btn>

      {onOpenSettings && (
        <Btn onClick={onOpenSettings} title={t('toolbar.settings')} aria-label={t('toolbar.settings')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
          </svg>
        </Btn>
      )}

      {!isPro && onOpenLicense && (
        <button
          className="toolbar-btn"
          onClick={onOpenLicense}
          title={t('license.upgrade')}
          aria-label={t('license.upgrade')}
          style={{ color: 'var(--color-accent)', fontSize: 11, fontWeight: 600, gap: 4 }}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
          </svg>
          <span className="btn-label">Pro</span>
        </button>
      )}

    </header>
    </>
  )
}

function Btn({ children, onClick, accent, active, disabled, title, 'aria-label': ariaLabel }: {
  children: React.ReactNode; onClick: () => void; accent?: boolean; active?: boolean; disabled?: boolean; title?: string; 'aria-label'?: string
}) {
  const cls = accent
    ? 'toolbar-btn toolbar-btn-accent'
    : active
      ? 'toolbar-btn toolbar-btn-active'
      : 'toolbar-btn'
  return (
    <button onClick={disabled ? undefined : onClick} title={title} aria-label={ariaLabel} className={cls} disabled={disabled} style={disabled ? { opacity: 0.35, cursor: 'not-allowed' } : undefined}>
      {children}
    </button>
  )
}
