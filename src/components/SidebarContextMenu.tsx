import { useEffect, useRef } from 'react'
import { useI18n } from '../lib/i18n'

interface ContextMenuProps {
  x: number
  y: number
  isDir: boolean
  fileName: string
  devMode: boolean
  isGitRepo: boolean
  gitBranch: string
  gitChangedCount: number
  gitInitTarget?: string
  onNewFile: () => void
  onNewFolder: () => void
  onRename: () => void
  onDelete: () => void
  onCopyPath: () => void
  onOpenInTerminal?: () => void
  onOpenGitPanel?: () => void
  onGitInit?: (targetDir: string) => void
  onClose: () => void
}

export function SidebarContextMenu({
  x, y, isDir, fileName, devMode, isGitRepo, gitBranch, gitChangedCount, gitInitTarget,
  onNewFile, onNewFolder, onRename, onDelete, onCopyPath,
  onOpenInTerminal, onOpenGitPanel, onGitInit, onClose,
}: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null)
  const { t } = useI18n()

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose()
    }
    const keyHandler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('mousedown', handler)
    document.addEventListener('keydown', keyHandler)
    return () => {
      document.removeEventListener('mousedown', handler)
      document.removeEventListener('keydown', keyHandler)
    }
  }, [onClose])

  type MenuItem = { label: string; action: () => void; danger?: boolean }
  type InfoItem = { info: string }
  const items: (MenuItem | InfoItem | 'separator')[] = []

  // File/folder creation
  items.push({ label: t('ctx.newFile'), action: onNewFile })
  items.push({ label: t('ctx.newFolder'), action: onNewFolder })

  // File/folder specific operations
  if (fileName) {
    items.push('separator')
    items.push({ label: t('ctx.rename'), action: onRename })
    items.push({ label: t('ctx.moveToTrash'), action: onDelete, danger: true })
  }

  // Git section
  items.push('separator')
  if (isGitRepo) {
    items.push({ info: `⎇ ${gitBranch || 'unknown'}${gitChangedCount > 0 ? ` · ${t('ctx.changes', { n: gitChangedCount })}` : ''}` })
  }
  if (onOpenGitPanel) {
    items.push({ label: t('ctx.gitOps'), action: onOpenGitPanel })
  } else if (!isGitRepo && onGitInit && gitInitTarget) {
    const dirName = gitInitTarget.split('/').pop() || gitInitTarget
    items.push({ label: t('ctx.gitInit', { dir: dirName }), action: () => onGitInit(gitInitTarget) })
  }

  // Dev mode extras
  if (devMode && onOpenInTerminal) {
    items.push('separator')
    items.push({ label: t('ctx.openInTerminal'), action: onOpenInTerminal })
  }

  // Copy path
  items.push('separator')
  items.push({ label: t('ctx.copyPath'), action: onCopyPath })

  return (
    <div
      ref={ref}
      className="ctx-menu"
      style={{ left: x, top: y }}
    >
      {items.map((item, i) => {
        if (item === 'separator') {
          return <div key={`sep-${i}`} className="ctx-menu-sep" />
        }
        if ('info' in item) {
          return (
            <div key={`info-${i}`} className="ctx-menu-info">
              {item.info}
            </div>
          )
        }
        return (
          <button
            key={item.label}
            className={`ctx-menu-item${item.danger ? ' ctx-menu-danger' : ''}`}
            onClick={() => { item.action(); onClose() }}
          >
            {item.label}
          </button>
        )
      })}
    </div>
  )
}
