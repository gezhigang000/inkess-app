import { useState, useRef, useEffect, useMemo, useCallback, type CSSProperties } from 'react'
import { List } from 'react-window'
import { listDirectory, type FileEntry } from '../lib/tauri'
import { isSupported, getFileType, type FileType } from '../lib/fileTypes'
import { SidebarContextMenu } from './SidebarContextMenu'
import { useI18n } from '../lib/i18n'

const MAX_DEPTH = 8
const MAX_ENTRIES_PER_LAYER = 500
const ROW_HEIGHT = 30
const INDENT_PX = 16

interface TreeNode {
  name: string
  path: string        // relative path from currentDir, e.g. "src/foo.ts"
  is_dir: boolean
  depth: number
  expanded: boolean
  loaded: boolean
  loading: boolean
  children: TreeNode[]
  truncated: number   // how many entries were cut off
}

interface SidebarProps {
  open: boolean
  currentFile: string
  currentFilePath: string
  files: FileEntry[]
  currentDir: string
  onSelectFile: (absolutePath: string) => void
  onNewFile: () => void
  onNewFolder: () => void
  onRename: (oldRelPath: string, newName: string) => void
  onDelete: (relPath: string) => void
  onCopyPath: (relPath: string) => void
  devMode: boolean
  isGitRepo: boolean
  gitBranch: string
  gitChangedCount: number
  onOpenInTerminal?: () => void
  onGitInit?: (targetDir: string) => void
  onOpenGitPanel?: () => void
  onSwitchDir?: (absolutePath: string) => void
}

function FileIcon({ type, isActive }: { type: FileType; isActive: boolean }) {
  const opacity = isActive ? 1 : 0.6
  const cls = "w-4 h-4 shrink-0"
  switch (type) {
    case 'image':
      return (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className={cls} style={{ opacity }}><rect x="3" y="3" width="18" height="18" rx="2" /><circle cx="8.5" cy="8.5" r="1.5" /><polyline points="21 15 16 10 5 21" /></svg>)
    case 'pdf':
      return (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className={cls} style={{ opacity }}><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /><path d="M9 15h6" /><path d="M9 11h6" /></svg>)
    case 'docx':
      return (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className={cls} style={{ opacity }}><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /><line x1="16" y1="13" x2="8" y2="13" /><line x1="16" y1="17" x2="8" y2="17" /><line x1="10" y1="9" x2="8" y2="9" /></svg>)
    case 'code':
      return (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className={cls} style={{ opacity }}><polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" /></svg>)
    default:
      return (<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className={cls} style={{ opacity }}><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /></svg>)
  }
}

function entriesToNodes(entries: FileEntry[], parentPath: string, depth: number): { nodes: TreeNode[]; truncated: number } {
  const limited = entries.slice(0, MAX_ENTRIES_PER_LAYER)
  const truncated = Math.max(0, entries.length - MAX_ENTRIES_PER_LAYER)
  const nodes: TreeNode[] = limited.map(e => ({
    name: e.name,
    path: parentPath ? `${parentPath}/${e.name}` : e.name,
    is_dir: e.is_dir,
    depth,
    expanded: false,
    loaded: false,
    loading: false,
    children: [],
    truncated: 0,
  }))
  return { nodes, truncated }
}

/** Flatten expanded tree into a list for virtual scrolling */
function flattenTree(nodes: TreeNode[]): TreeNode[] {
  const result: TreeNode[] = []
  const walk = (list: TreeNode[]) => {
    for (const node of list) {
      result.push(node)
      if (node.is_dir && node.expanded && node.children.length > 0) {
        walk(node.children)
      }
    }
  }
  walk(nodes)
  return result
}

/** Find a node by path in the tree */
function findNode(nodes: TreeNode[], target: string): TreeNode | null {
  for (const n of nodes) {
    if (n.path === target) return n
    if (n.is_dir && target.startsWith(n.path + '/')) {
      const found = findNode(n.children, target)
      if (found) return found
    }
  }
  return null
}

/** Deep-update a node at the given path in the tree */
function updateNodeAtPath(
  nodes: TreeNode[],
  targetPath: string,
  updater: (node: TreeNode) => TreeNode
): TreeNode[] {
  return nodes.map(node => {
    if (node.path === targetPath) return updater(node)
    if (node.is_dir && targetPath.startsWith(node.path + '/')) {
      return { ...node, children: updateNodeAtPath(node.children, targetPath, updater) }
    }
    return node
  })
}

// Chevron for expand/collapse
function Chevron({ expanded, canExpand }: { expanded: boolean; canExpand: boolean }) {
  if (!canExpand) return <span className="w-3 h-3 shrink-0" />
  return (
    <svg
      viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
      className="w-3 h-3 shrink-0 transition-transform duration-150"
      style={{ transform: expanded ? 'rotate(90deg)' : 'rotate(0deg)', opacity: 0.5 }}
    >
      <polyline points="9 18 15 12 9 6" />
    </svg>
  )
}

interface RowData {
  flatNodes: TreeNode[]
  currentFilePath: string
  currentDir: string
  renamingPath: string | null
  renameValue: string
  onToggle: (path: string) => void
  onSelect: (node: TreeNode) => void
  onContextMenu: (e: React.MouseEvent, node: TreeNode) => void
  onRenameChange: (val: string) => void
  onRenameConfirm: () => void
  onRenameCancel: () => void
  renameRef: React.RefObject<HTMLInputElement | null>
  onSwitchDir?: (absolutePath: string) => void
}

function TreeRow({ index, style, ariaAttributes, flatNodes, currentFilePath, currentDir, renamingPath, renameValue, onToggle, onSelect, onContextMenu, onRenameChange, onRenameConfirm, onRenameCancel, renameRef, onSwitchDir }: { index: number; style: CSSProperties; ariaAttributes?: Record<string, unknown> } & RowData) {
  const node = flatNodes[index]
  if (!node) return null

  const absPath = currentDir + '/' + node.path
  const isActive = absPath === currentFilePath
  const fileType = node.is_dir ? 'markdown' as FileType : getFileType(node.name)
  const isRenaming = renamingPath === node.path
  const canExpand = node.is_dir && node.depth < MAX_DEPTH

  return (
    <div
      {...ariaAttributes}
      style={{
        ...style,
        paddingLeft: 10 + node.depth * INDENT_PX,
        paddingRight: 10,
      }}
      role="button"
      tabIndex={0}
      aria-current={isActive ? 'page' : undefined}
      className="sidebar-item flex items-center gap-1.5 text-[13px] cursor-pointer transition-all select-none"
      onClick={() => {
        if (isRenaming) return
        if (node.is_dir) onToggle(node.path)
        else onSelect(node)
      }}
      onDoubleClick={() => {
        if (isRenaming) return
        if (node.is_dir && onSwitchDir) onSwitchDir(absPath)
      }}
      onKeyDown={e => {
        if (e.key === 'Enter' && !isRenaming) {
          if (node.is_dir) onToggle(node.path)
          else onSelect(node)
        }
      }}
      onContextMenu={(e) => { e.preventDefault(); e.stopPropagation(); onContextMenu(e, node) }}
    >
      {node.is_dir ? (
        <>
          <Chevron expanded={node.expanded} canExpand={canExpand} />
          {node.loading ? (
            <svg className="w-4 h-4 shrink-0 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ opacity: 0.5 }}>
              <circle cx="12" cy="12" r="10" strokeDasharray="31.4 31.4" />
            </svg>
          ) : (
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-4 h-4 shrink-0" style={{ opacity: 0.6 }}>
              {node.expanded
                ? <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
                : <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
              }
            </svg>
          )}
        </>
      ) : (
        <>
          <span className="w-3 h-3 shrink-0" />
          <FileIcon type={fileType} isActive={isActive} />
        </>
      )}
      {isRenaming ? (
        <input
          ref={renameRef}
          className="rename-input"
          value={renameValue}
          onChange={e => onRenameChange(e.target.value)}
          onBlur={onRenameConfirm}
          onKeyDown={e => { if (e.key === 'Enter') onRenameConfirm(); if (e.key === 'Escape') onRenameCancel() }}
          onClick={e => e.stopPropagation()}
        />
      ) : (
        <span
          className="truncate"
          style={{
            color: isActive ? 'var(--color-accent)' : node.is_dir ? 'var(--text-3)' : 'var(--text-2)',
            fontWeight: isActive ? 500 : 400,
            background: isActive ? 'var(--accent-subtle)' : 'transparent',
            borderRadius: 3,
            padding: isActive ? '0 4px' : undefined,
          }}
        >
          {node.is_dir ? `${node.name}/` : node.name}
        </span>
      )}
    </div>
  )
}

export function Sidebar({
  open, currentFile, currentFilePath, files, currentDir,
  onSelectFile, onNewFile, onNewFolder, onRename, onDelete, onCopyPath,
  devMode, isGitRepo, gitBranch, gitChangedCount,
  onOpenInTerminal, onGitInit, onOpenGitPanel, onSwitchDir,
}: SidebarProps) {
  const { t } = useI18n()
  const [tree, setTree] = useState<TreeNode[]>([])
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; node: TreeNode | null } | null>(null)
  const [renamingPath, setRenamingPath] = useState<string | null>(null)
  const [renameValue, setRenameValue] = useState('')
  const renameRef = useRef<HTMLInputElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const [containerHeight, setContainerHeight] = useState(400)

  // Build root tree from files prop
  useEffect(() => {
    const { nodes } = entriesToNodes(files, '', 0)
    setTree(nodes)
  }, [files])

  // Measure container height
  useEffect(() => {
    if (!containerRef.current) return
    const ro = new ResizeObserver(entries => {
      for (const entry of entries) {
        setContainerHeight(entry.contentRect.height)
      }
    })
    ro.observe(containerRef.current)
    return () => ro.disconnect()
  }, [])

  // Focus rename input
  useEffect(() => {
    if (renamingPath && renameRef.current) {
      renameRef.current.focus()
      const dotIdx = renameValue.lastIndexOf('.')
      renameRef.current.setSelectionRange(0, dotIdx > 0 ? dotIdx : renameValue.length)
    }
  }, [renamingPath])

  const toggleExpand = useCallback((nodePath: string) => {
    setTree(prev => {
      const node = findNode(prev, nodePath)
      if (!node || node.depth >= MAX_DEPTH) return prev

      // If already expanded, collapse
      if (node.expanded) {
        return updateNodeAtPath(prev, nodePath, n => ({ ...n, expanded: false }))
      }

      // If already loaded, just expand
      if (node.loaded) {
        return updateNodeAtPath(prev, nodePath, n => ({ ...n, expanded: true }))
      }

      // Need to load — mark loading and trigger async load
      const absDir = currentDir + '/' + nodePath
      listDirectory(absDir).then(result => {
        const { nodes: children, truncated } = entriesToNodes(result.entries, nodePath, node.depth + 1)
        setTree(prevTree =>
          updateNodeAtPath(prevTree, nodePath, n => ({
            ...n,
            children,
            truncated,
            loaded: true,
            loading: false,
            expanded: true,
          }))
        )
      }).catch(() => {
        setTree(prevTree =>
          updateNodeAtPath(prevTree, nodePath, n => ({ ...n, loading: false }))
        )
      })

      return updateNodeAtPath(prev, nodePath, n => ({ ...n, loading: true }))
    })
  }, [currentDir])

  const handleSelect = useCallback((node: TreeNode) => {
    if (!currentDir) return
    onSelectFile(currentDir + '/' + node.path)
  }, [currentDir, onSelectFile])

  const handleContextMenu = useCallback((e: React.MouseEvent, node: TreeNode) => {
    setCtxMenu({ x: e.clientX, y: e.clientY, node })
  }, [])

  const confirmRename = useCallback(() => {
    if (renamingPath && renameValue.trim() && renameValue !== renamingPath.split('/').pop()) {
      onRename(renamingPath, renameValue.trim())
    }
    setRenamingPath(null)
  }, [renamingPath, renameValue, onRename])

  const flatNodes = useMemo(() => flattenTree(tree), [tree])
  const hasFiles = files.length > 0

  const rowData: RowData = useMemo(() => ({
    flatNodes,
    currentFilePath,
    currentDir,
    renamingPath,
    renameValue,
    onToggle: toggleExpand,
    onSelect: handleSelect,
    onContextMenu: handleContextMenu,
    onRenameChange: setRenameValue,
    onRenameConfirm: confirmRename,
    onRenameCancel: () => setRenamingPath(null),
    renameRef,
    onSwitchDir,
  }), [flatNodes, currentFilePath, currentDir, renamingPath, renameValue, toggleExpand, handleSelect, handleContextMenu, confirmRename, onSwitchDir])

  const RowComponent = useCallback(
    (props: { index: number; style: CSSProperties; ariaAttributes: Record<string, unknown> } & RowData) => {
      return <TreeRow {...props} />
    },
    []
  )

  return (
    <nav
      aria-label={t('sidebar.fileBrowser')}
      className="flex flex-col transition-all duration-250 ease-in-out"
      style={{
        flex: 1, overflow: 'hidden',
        opacity: open ? 1 : 0, padding: open ? '12px 0' : 0,
      }}
      onContextMenu={(e) => { e.preventDefault(); setCtxMenu({ x: e.clientX, y: e.clientY, node: null }) }}
    >
      <div className="flex items-center" style={{ padding: '4px 10px 8px' }}>
        <span className="text-[11px] font-semibold uppercase" style={{ color: 'var(--text-3)', letterSpacing: '0.06em' }}>{t('sidebar.files')}</span>
      </div>

      {!hasFiles ? (
        <div className="px-2.5 py-6 text-center text-[12.5px]" style={{ color: 'var(--text-3)' }}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="w-8 h-8 mx-auto mb-2 opacity-40"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /></svg>
          {t('sidebar.empty')}
        </div>
      ) : (
        <div ref={containerRef} style={{ flex: 1, overflow: 'hidden' }}>
          <List<RowData>
            rowCount={flatNodes.length}
            rowHeight={ROW_HEIGHT}
            rowComponent={RowComponent}
            rowProps={rowData}
            style={{ height: containerHeight }}
            overscanCount={10}
          />
        </div>
      )}

      {ctxMenu && (
        <SidebarContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          isDir={ctxMenu.node?.is_dir ?? true}
          fileName={ctxMenu.node?.name ?? ''}
          devMode={devMode}
          isGitRepo={isGitRepo}
          gitBranch={gitBranch}
          gitChangedCount={gitChangedCount}
          onNewFile={onNewFile}
          onNewFolder={onNewFolder}
          onRename={() => {
            if (ctxMenu.node) {
              setRenamingPath(ctxMenu.node.path)
              setRenameValue(ctxMenu.node.name)
            }
          }}
          onDelete={() => { if (ctxMenu.node) onDelete(ctxMenu.node.path) }}
          onCopyPath={() => { if (ctxMenu.node) onCopyPath(ctxMenu.node.path); else onCopyPath('') }}
          onOpenInTerminal={onOpenInTerminal}
          onOpenGitPanel={onOpenGitPanel}
          onGitInit={onGitInit}
          gitInitTarget={
            ctxMenu.node?.is_dir
              ? currentDir + '/' + ctxMenu.node.path
              : currentDir
          }
          onClose={() => setCtxMenu(null)}
        />
      )}
    </nav>
  )
}
