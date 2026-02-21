import { useState, useEffect } from 'react'
import type { SnapshotInfo } from '../lib/tauri'
import { useI18n } from '../lib/i18n'

interface TimelineProps {
  open: boolean
  snapshots: SnapshotInfo[]
  activeSnapshotId: number | null
  onSelectSnapshot: (id: number | null) => void
}

function formatTime(isoStr: string, t: (key: string, vars?: Record<string, string | number>) => string): string {
  const date = new Date(isoStr)
  if (isNaN(date.getTime())) return t('time.unknown')
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMin = Math.floor(diffMs / 60000)
  const diffHr = Math.floor(diffMs / 3600000)
  const diffDay = Math.floor(diffMs / 86400000)

  if (diffMin < 1) return t('time.justNow')
  if (diffMin < 60) return t('time.minutesAgo', { n: diffMin })
  if (diffHr < 24) return t('time.hoursAgo', { n: diffHr })
  if (diffDay < 7) return t('time.daysAgo', { n: diffDay })
  return date.toLocaleDateString('zh-CN', {
    month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit',
  })
}

const INITIAL_SHOW = 20

export function Timeline({
  open, snapshots, activeSnapshotId, onSelectSnapshot,
}: TimelineProps) {
  const [expanded, setExpanded] = useState(false)
  const { t } = useI18n()
  const isLatest = activeSnapshotId === null

  // Reset expand when switching files
  useEffect(() => { setExpanded(false) }, [snapshots])
  const displaySnapshots = expanded ? snapshots : snapshots.slice(0, INITIAL_SHOW)
  const hasMore = snapshots.length > INITIAL_SHOW && !expanded

  return (
    <div
      role="navigation"
      aria-label={t('timeline.nav')}
      className="flex items-center px-5 gap-3 shrink-0
        transition-all duration-250 overflow-hidden timeline-responsive"
      style={{
        height: open ? 48 : 0,
        opacity: open ? 1 : 0,
        borderTop: open ? '1px solid var(--border-s)' : 'none',
        background: 'var(--sidebar-bg)',
      }}
    >
      <div
        className="text-[11px] font-semibold uppercase
          whitespace-nowrap timeline-label-responsive"
        style={{ color: 'var(--text-3)', letterSpacing: '0.05em' }}
      >
        {t('timeline.label')}
      </div>

      {displaySnapshots.length === 0 ? (
        <div className="text-[11.5px]" style={{ color: 'var(--text-3)' }}>
          {t('timeline.noRecords')}
        </div>
      ) : (
        <>
          <div className="flex items-center gap-1.5 overflow-x-auto flex-1">
            <button
              className={`timeline-dot ${isLatest ? 'timeline-dot-active' : ''}`}
              onClick={() => onSelectSnapshot(null)}
              title={t('timeline.current')}
              aria-label={t('timeline.current')}
            />
            {displaySnapshots.map((snap) => (
              <div key={snap.id} className="flex items-center gap-1.5">
                <div className="w-6 h-px shrink-0" style={{ background: 'var(--border)' }} />
                <button
                  className={`timeline-dot ${activeSnapshotId === snap.id ? 'timeline-dot-active' : ''}`}
                  onClick={() => onSelectSnapshot(snap.id)}
                  title={formatTime(snap.created_at, t)}
                  aria-label={formatTime(snap.created_at, t)}
                />
              </div>
            ))}
            {hasMore && (
              <button
                className="text-[11px] whitespace-nowrap px-2 py-0.5 rounded cursor-pointer border-none"
                style={{ background: 'var(--accent-subtle)', color: 'var(--color-accent)', fontFamily: 'inherit' }}
                onClick={() => setExpanded(true)}
              >
                {t('timeline.more', { count: snapshots.length - INITIAL_SHOW })}
              </button>
            )}
          </div>

          <div className="text-[11.5px] whitespace-nowrap timeline-info-responsive" style={{ color: 'var(--text-2)' }}>
            {isLatest ? (
              <span style={{ color: 'var(--text)' }}>{t('timeline.current')}</span>
            ) : (
              <>
                <span className="font-medium" style={{ color: 'var(--color-accent)' }}>{t('timeline.history')}</span>
                {' · '}
                {formatTime(displaySnapshots.find(s => s.id === activeSnapshotId)?.created_at || '', t)}
              </>
            )}
            <span style={{ color: 'var(--text-3)' }}> · {t('timeline.snapshots', { count: snapshots.length })}</span>
          </div>
        </>
      )}
    </div>
  )
}
