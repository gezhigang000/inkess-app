import { useI18n } from '../lib/i18n'

interface ConflictDialogProps {
  fileName: string
  onKeepMine: () => void
  onAcceptExternal: () => void
  onDismiss: () => void
}

export function ConflictDialog({ fileName, onKeepMine, onAcceptExternal, onDismiss }: ConflictDialogProps) {
  const { t } = useI18n()
  return (
    <div className="shortcuts-backdrop" onClick={onDismiss}>
      <div className="shortcuts-modal" onClick={e => e.stopPropagation()} style={{ minWidth: 360 }}>
        <div className="flex items-center justify-between mb-1">
          <h3 style={{ margin: 0 }}>{t('conflict.title')}</h3>
          <button className="sidebar-action-btn" onClick={onDismiss} aria-label={t('ai.close')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
        <p className="text-[13px] mb-5" style={{ color: 'var(--text-2)', lineHeight: '1.6' }}>
          {t('conflict.message', { name: fileName })}
        </p>
        <div className="flex flex-col gap-2.5">
          <button className="toolbar-btn w-full justify-center" onClick={() => { onKeepMine(); onDismiss() }}>
            {t('conflict.keepMine')}
          </button>
          <button className="toolbar-btn w-full justify-center" onClick={() => { onAcceptExternal(); onDismiss() }}>
            {t('conflict.acceptExternal')}
          </button>
          <button
            className="text-[12px] py-2 cursor-pointer bg-transparent border-none"
            style={{ color: 'var(--text-3)' }}
            onClick={onDismiss}
          >
            {t('conflict.later')}
          </button>
        </div>
      </div>
    </div>
  )
}
