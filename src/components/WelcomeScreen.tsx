import { useI18n } from '../lib/i18n'

interface WelcomeScreenProps {
  recentDirs: string[]
  onOpenDir: (path: string) => void
  onOpen: () => void
}

export function WelcomeScreen({ recentDirs, onOpenDir, onOpen }: WelcomeScreenProps) {
  const { t } = useI18n()
  return (
    <div className="welcome-screen">
      <div className="welcome-logo">Inkess</div>
      <div className="welcome-tagline">{t('welcome.tagline')}</div>

      {recentDirs.length > 0 ? (
        <div className="welcome-recent">
          <div className="welcome-recent-title">{t('welcome.recent')}</div>
          {recentDirs.map(dir => {
            const name = dir.split('/').filter(Boolean).pop() || dir
            return (
              <div
                key={dir}
                className="welcome-recent-item"
                onClick={() => onOpenDir(dir)}
                role="button"
                tabIndex={0}
                onKeyDown={e => { if (e.key === 'Enter') onOpenDir(dir) }}
              >
                <span className="welcome-recent-name">{name}</span>
                <span className="welcome-recent-path">{dir}</span>
              </div>
            )
          })}
        </div>
      ) : (
        <div className="welcome-recent">
          <div style={{ textAlign: 'center', color: 'var(--text-3)', fontSize: 13, padding: '16px 0' }}>
            {t('welcome.emptyHint')}
          </div>
        </div>
      )}

      <button className="toolbar-btn toolbar-btn-accent" onClick={onOpen}>
        {t('welcome.openFolder')}
      </button>
      <div className="welcome-hint" dangerouslySetInnerHTML={{ __html: t('welcome.quickOpen') }} />
    </div>
  )
}
