import { type GitFileStatus } from '../lib/tauri'
import { useI18n } from '../lib/i18n'

interface GitStatusListProps {
  files: GitFileStatus[]
  onToggleStage: (path: string, staged: boolean) => void
}

const STATUS_COLORS: Record<string, string> = {
  'M': '#ca8a04', // yellow
  'A': '#16a34a', // green
  'D': '#dc2626', // red
  '?': '#9ca3af', // gray
  'R': '#2563eb', // blue
}

export function GitStatusList({ files, onToggleStage }: GitStatusListProps) {
  const { t } = useI18n()

  if (files.length === 0) {
    return (
      <div className="px-2 py-3 text-[12px] text-center" style={{ color: 'var(--text-3)' }}>
        {t('git.noChanges')}
      </div>
    )
  }

  const staged = files.filter(f => f.staged)
  const unstaged = files.filter(f => !f.staged)

  return (
    <div className="git-status-list">
      {staged.length > 0 && (
        <>
          <div className="git-section-label">{t('git.staged')}</div>
          {staged.map(f => (
            <div key={`s-${f.path}`} className="git-file-item" onClick={() => onToggleStage(f.path, true)}>
              <span className="git-status-badge" style={{ color: STATUS_COLORS[f.status] || 'var(--text-3)' }}>{f.status}</span>
              <span className="truncate text-[12px]">{f.path}</span>
            </div>
          ))}
        </>
      )}
      {unstaged.length > 0 && (
        <>
          <div className="git-section-label">{t('git.unstaged')}</div>
          {unstaged.map(f => (
            <div key={`u-${f.path}`} className="git-file-item" onClick={() => onToggleStage(f.path, false)}>
              <span className="git-status-badge" style={{ color: STATUS_COLORS[f.status] || 'var(--text-3)' }}>{f.status}</span>
              <span className="truncate text-[12px]">{f.path}</span>
            </div>
          ))}
        </>
      )}
    </div>
  )
}
