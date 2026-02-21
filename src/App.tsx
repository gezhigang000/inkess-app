import { useState, useCallback, useEffect, useRef, useTransition } from 'react'
import { Toolbar } from './components/Toolbar'
import { Sidebar } from './components/Sidebar'
import { ContentArea } from './components/ContentArea'
import { Timeline } from './components/Timeline'
import { Toast } from './components/Toast'
import { NewFilePopup } from './components/NewFilePopup'
import { ConflictDialog } from './components/ConflictDialog'
import { TerminalPanel } from './components/TerminalPanel'
import { GitPanel } from './components/GitPanel'
import { WelcomeScreen } from './components/WelcomeScreen'
import { AIChatPanel } from './components/AIChatPanel'
import { SettingsPanel } from './components/SettingsPanel'
import { LicensePanel } from './components/LicensePanel'
import { UpgradePrompt } from './components/UpgradePrompt'
import { FindBar } from './components/FindBar'
import { DevConsole } from './components/DevConsole'
import { I18nProvider, useI18n } from './lib/i18n'
import { LicenseProvider, useLicense } from './lib/license'
import {
  readFile, readFileBinary, saveFile, listDirectory, createSnapshot,
  listSnapshots, getSnapshotContent, openFileOrDirDialog, getInitialFile,
  createFile, createDirectory, renameEntry, deleteToTrash,
  watchDirectory, gitStatus, gitInit, loadSettings, saveSettings, getFileSize,
  ragInit, preloadPythonEnv,
  type FileEntry, type SnapshotInfo,
} from './lib/tauri'
import { type ThemeId } from './lib/themes'
import { exportFile } from './lib/export'
import { type ContentData } from './types/content'
import { getFileType, getLanguage, isEditable, isTextBased, isSupported } from './lib/fileTypes'
import { convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { homeDir as getHomeDir } from '@tauri-apps/api/path'
import './themes/github.css'
import './themes/minimal.css'
import './themes/dark.css'

const DEMO_MARKDOWN = `# Welcome to Inkess

Inkess is a lightweight Markdown reader and format conversion tool. **Simple, organized** — focus on your content.

> Simple, organized.

## Quick Start

Just double-click any \`.md\` file and Inkess will render it beautifully. Core features:

- Beautiful Markdown rendering with multiple themes
- One-click export to **PDF**, **Word**, **HTML**
- Automatic version history, revert anytime
- File browser for quick document switching
- Multi-format preview (code, images, PDF, Word)

## Shortcuts

| Shortcut | Action |
|----------|--------|
| ⌘ O | Open file |
| ⌘ E | Toggle edit/read |
| ⌘ S | Save |
| ⌘ ⇧ E | Quick export PDF |
| Esc | Exit editing |

## Code Highlighting

\`\`\`rust
fn main() {
    let greeting = "Simple, organized";
    println!("{}", greeting);
}
\`\`\`

---

Drop files or press ⌘O to get started. Supports Markdown, text, code, images, PDF, and Word files.
`

const DEMO_CONTENT: ContentData = { type: 'markdown', text: DEMO_MARKDOWN }
const TOAST_DURATION = 2500
const AUTOSAVE_DELAY = 3000

function loadTheme(): ThemeId {
  const saved = localStorage.getItem('inkess-theme')
  if (saved === 'github' || saved === 'minimal' || saved === 'dark') return saved
  return 'github'
}

function loadRecentDirs(): string[] {
  try {
    const raw = localStorage.getItem('inkess-recent-dirs')
    if (raw) return (JSON.parse(raw) as string[]).slice(0, 5)
  } catch { /* ignore */ }
  return []
}

/** Extract text from ContentData (for text-based types) */
function getContentText(content: ContentData): string {
  if ('text' in content) return content.text
  return ''
}

export default function App() {
  return (
    <I18nProvider>
      <LicenseProvider>
        <AppInner />
      </LicenseProvider>
    </I18nProvider>
  )
}

function AppInner() {
  const { t } = useI18n()
  const { isPro } = useLicense()
  const [themeId, setThemeId] = useState<ThemeId>(loadTheme)
  const [sidebarOpen, setSidebarOpen] = useState(true)
  const [timelineOpen, setTimelineOpen] = useState(true)
  const [toast, setToast] = useState('')
  const [currentFile, setCurrentFile] = useState('')
  const [currentDir, setCurrentDir] = useState('')
  const [currentFilePath, setCurrentFilePath] = useState('')
  const [content, setContent] = useState<ContentData>(DEMO_CONTENT)
  const [latestText, setLatestText] = useState(DEMO_MARKDOWN)
  const [files, setFiles] = useState<FileEntry[]>([])
  const [snapshots, setSnapshots] = useState<SnapshotInfo[]>([])
  const [activeSnapshotId, setActiveSnapshotId] = useState<number | null>(null)
  const [editing, setEditing] = useState(false)
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false)
  const [loading, setLoading] = useState(false)
  const [dragging, setDragging] = useState(false)
  const [isReadOnly, setIsReadOnly] = useState(false)
  const [newFilePopup, setNewFilePopup] = useState<'file' | 'folder' | null>(null)
  const [conflictFile, setConflictFile] = useState<string | null>(null)
  const [devMode, setDevMode] = useState(() => localStorage.getItem('inkess-devmode') === 'true')
  const [terminalVisible, setTerminalVisible] = useState(true)
  const [isGitRepo, setIsGitRepo] = useState(false)
  const [gitBranch, setGitBranch] = useState('')
  const [gitChangedCount, setGitChangedCount] = useState(0)
  const [homeDir, setHomeDir] = useState('/')
  const [gitPanelOpen, setGitPanelOpen] = useState(false)
  const [gitInitConfirm, setGitInitConfirm] = useState<string | null>(null)
  const [recentDirs, setRecentDirs] = useState<string[]>(loadRecentDirs)
  const [aiPanelOpen, setAiPanelOpen] = useState(false)
  const aiBusyRef = useRef(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [licensePanelOpen, setLicensePanelOpen] = useState(false)
  const [findBarOpen, setFindBarOpen] = useState(false)
  const [devConsoleOpen, setDevConsoleOpen] = useState(false)
  const [showAbout, setShowAbout] = useState(false)

  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const dragCountRef = useRef(0)
  const openRequestRef = useRef(0)
  const openFileRef = useRef<(path: string) => Promise<void>>(null!)
  const handleOpenRef = useRef<() => void>(null!)
  const handleSaveRef = useRef<() => void>(null!)
  const toggleEditRef = useRef<() => void>(null!)
  const toggleDevModeRef = useRef<() => void>(null!)
  const currentFileRef = useRef(currentFile)
  const currentFilePathRef = useRef(currentFilePath)
  const hasUnsavedRef = useRef(hasUnsavedChanges)
  const [, startTransition] = useTransition()
  const prevBlobUrlRef = useRef<string | null>(null)
  const isDark = themeId === 'dark'
  const isViewingHistory = activeSnapshotId !== null
  const currentFileType = currentFile ? getFileType(currentFile) : 'markdown'
  const canEdit = isEditable(currentFileType) && !isReadOnly
  const canSnapshot = isTextBased(currentFileType)
  const canExport = currentFileType === 'markdown'
  const isHtmlPreview = currentFileType === 'html' && !editing

  const showToast = useCallback((msg: string, duration = TOAST_DURATION) => {
    setToast(msg)
    if (toastTimerRef.current) clearTimeout(toastTimerRef.current)
    toastTimerRef.current = setTimeout(() => setToast(''), duration)
  }, [])

  // Keep refs in sync
  currentFileRef.current = currentFile
  currentFilePathRef.current = currentFilePath
  hasUnsavedRef.current = hasUnsavedChanges

  // Load theme from settings.json on mount
  useEffect(() => {
    loadSettings().then(s => {
      if (s.theme === 'github' || s.theme === 'minimal' || s.theme === 'dark') {
        setThemeId(s.theme as ThemeId)
        localStorage.setItem('inkess-theme', s.theme)
      }
    }).catch(() => {})
  }, [])

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : '')
    localStorage.setItem('inkess-theme', themeId)
    // Persist to settings.json
    loadSettings().then(s => saveSettings({ ...s, theme: themeId })).catch(() => {})
  }, [isDark, themeId])

  // Get home directory for terminal default cwd
  useEffect(() => {
    getHomeDir().then(dir => setHomeDir(dir)).catch(() => {})
  }, [])

  const dirRequestRef = useRef(0)

  useEffect(() => {
    if (!currentDir) return
    const reqId = ++dirRequestRef.current
    listDirectory(currentDir).then(result => {
      if (reqId === dirRequestRef.current) {
        startTransition(() => setFiles(result.entries))
      }
    }).catch(() => {})
  }, [currentDir])

  // Update recent dirs when currentDir changes
  useEffect(() => {
    if (!currentDir) return
    setRecentDirs(prev => {
      const updated = [currentDir, ...prev.filter(d => d !== currentDir)].slice(0, 5)
      localStorage.setItem('inkess-recent-dirs', JSON.stringify(updated))
      return updated
    })
  }, [currentDir])

  const refreshSnapshots = useCallback(async (filePath: string) => {
    if (!filePath) { setSnapshots([]); return }
    try {
      const snaps = await listSnapshots(filePath)
      setSnapshots(isPro ? snaps : snaps.slice(0, 10))
    } catch { setSnapshots([]) }
  }, [isPro])

  // Refresh file list helper
  const refreshFiles = useCallback(async () => {
    if (!currentDir) return
    try {
      const result = await listDirectory(currentDir)
      startTransition(() => setFiles(result.entries))
    } catch { /* silent */ }
  }, [currentDir])

  // File watcher: watch currentDir and handle fs-changed events (delayed to avoid blocking)
  useEffect(() => {
    if (!currentDir) return
    const watchTimer = setTimeout(() => {
      watchDirectory(currentDir).catch(() => {})
    }, 1000)
    let unlisten: (() => void) | undefined
    listen<{ path: string; kind: string }>('fs-changed', (event) => {
      const { path: changedPath, kind } = event.payload
      if (kind === 'create' || kind === 'remove') {
        refreshFiles()
      } else if (kind === 'modify') {
        if (currentFilePathRef.current && changedPath === currentFilePathRef.current) {
          if (hasUnsavedRef.current) {
            setConflictFile(currentFileRef.current)
          } else {
            readFile(currentFilePathRef.current).then(text => {
              const ft = getFileType(currentFileRef.current)
              if (ft === 'markdown') setContent({ type: 'markdown', text })
              else if (ft === 'code') setContent({ type: 'code', text, language: getLanguage(currentFileRef.current) || 'plaintext' })
              else if (ft === 'html') setContent({ type: 'html', text })
              else setContent({ type: 'text', text })
              setLatestText(text)
            }).catch(() => {})
          }
        }
      }
    }).then(fn => { unlisten = fn })
    return () => { unlisten?.(); clearTimeout(watchTimer) }
  }, [currentDir, refreshFiles])

  // File operation handlers
  const handleCreateFile = useCallback(async (name: string, template?: string) => {
    if (!currentDir) return
    try {
      await createFile(currentDir + '/' + name, template || '')
      await refreshFiles()
      showToast(t('toast.created', { name }))
    } catch (e) {
      showToast(typeof e === 'string' ? e : t('toast.createFailed'))
    }
    setNewFilePopup(null)
  }, [currentDir, refreshFiles, showToast, t])

  const handleCreateFolder = useCallback(async (name: string) => {
    if (!currentDir) return
    try {
      await createDirectory(currentDir + '/' + name)
      await refreshFiles()
      showToast(t('toast.createdFolder', { name }))
    } catch (e) {
      showToast(typeof e === 'string' ? e : t('toast.createFailed'))
    }
    setNewFilePopup(null)
  }, [currentDir, refreshFiles, showToast, t])

  const handleRenameEntry = useCallback(async (oldRelPath: string, newName: string) => {
    if (!currentDir) return
    const oldAbsPath = currentDir + '/' + oldRelPath
    const parentRel = oldRelPath.includes('/') ? oldRelPath.substring(0, oldRelPath.lastIndexOf('/') + 1) : ''
    const newAbsPath = currentDir + '/' + parentRel + newName
    try {
      await renameEntry(oldAbsPath, newAbsPath)
      await refreshFiles()
      const oldName = oldRelPath.split('/').pop()!
      if (currentFile === oldName) setCurrentFile(newName)
      showToast(t('toast.renamed'))
    } catch (e) {
      showToast(typeof e === 'string' ? e : t('toast.renameFailed'))
    }
  }, [currentDir, currentFile, refreshFiles, showToast, t])

  const handleDeleteEntry = useCallback(async (name: string) => {
    if (!currentDir) return
    try {
      await deleteToTrash(currentDir + '/' + name)
      await refreshFiles()
      showToast(t('toast.movedToTrash'))
    } catch (e) {
      showToast(typeof e === 'string' ? e : t('toast.deleteFailed'))
    }
  }, [currentDir, refreshFiles, showToast, t])

  const handleCopyPath = useCallback((name: string) => {
    if (!currentDir) return
    navigator.clipboard.writeText(currentDir + '/' + name).then(() => showToast(t('toast.copiedPath')))
  }, [currentDir, showToast, t])

  const toggleDevMode = useCallback(() => {
    setDevMode(v => {
      const next = !v
      localStorage.setItem('inkess-devmode', String(next))
      if (next) setTerminalVisible(true)
      return next
    })
  }, [])

  // Check git repo status when directory changes (only in dev mode)
  useEffect(() => {
    if (!currentDir || !devMode) {
      setIsGitRepo(false)
      setGitBranch('')
      setGitChangedCount(0)
      return
    }
    const timer = setTimeout(() => {
      gitStatus(currentDir).then(s => {
        startTransition(() => {
          setIsGitRepo(s.is_repo)
          setGitBranch(s.branch || '')
          setGitChangedCount(s.files?.length || 0)
        })
      }).catch(() => {
        setIsGitRepo(false)
        setGitBranch('')
        setGitChangedCount(0)
      })
    }, 1500)
    return () => clearTimeout(timer)
  }, [currentDir, devMode])

  const handleGitInit = useCallback((targetDir: string) => {
    setGitInitConfirm(targetDir)
  }, [])

  const confirmGitInit = useCallback(async () => {
    if (!gitInitConfirm) return
    const targetDir = gitInitConfirm
    setGitInitConfirm(null)
    try {
      await gitInit(targetDir)
      setIsGitRepo(true)
      showToast(t('toast.gitInited'))
      // Refresh git status
      const s = await gitStatus(currentDir || targetDir)
      setIsGitRepo(s.is_repo)
      setGitBranch(s.branch || '')
      setGitChangedCount(s.files?.length || 0)
    } catch (e) {
      showToast(typeof e === 'string' ? e : t('toast.gitInitFailed'))
    }
  }, [gitInitConfirm, currentDir, showToast, t])

  const handleSave = useCallback(async () => {
    if (!currentFilePath || !hasUnsavedChanges || !canEdit) return
    const text = getContentText(content)
    try {
      await saveFile(currentFilePath, text)
      setLatestText(text)
      setHasUnsavedChanges(false)
      if (canSnapshot) {
        await createSnapshot(currentFilePath, text).catch(() => {})
        await refreshSnapshots(currentFilePath)
      }
      showToast(t('toast.saved'))
    } catch {
      showToast(t('toast.saveFailed'), 4000)
    }
  }, [currentFilePath, content, hasUnsavedChanges, canEdit, canSnapshot, showToast, refreshSnapshots, t])

  const openFile = useCallback(async (filePath: string) => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current)
      saveTimerRef.current = null
    }
    // Auto-save previous text-based file
    if (hasUnsavedChanges && currentFilePath && isTextBased(getFileType(currentFile))) {
      const prevText = getContentText(content)
      try {
        await saveFile(currentFilePath, prevText)
        await createSnapshot(currentFilePath, prevText).catch(() => {})
      } catch {
        showToast(t('toast.autoSaveFailed'))
      }
    }
    const reqId = ++openRequestRef.current
    setLoading(true)
    try {
      const parts = filePath.split('/')
      const fileName = parts[parts.length - 1]
      const fileType = getFileType(fileName)

      // Large file warning (>5MB)
      try {
        const size = await getFileSize(filePath)
        if (size > 5 * 1024 * 1024) {
          const sizeMB = (size / 1024 / 1024).toFixed(1)
          const ok = window.confirm(t('toast.largeFile', { size: sizeMB }))
          if (!ok) { setLoading(false); return }
          if (reqId !== openRequestRef.current) { setLoading(false); return }
        }
      } catch { /* ignore size check errors, proceed with open */ }

      let newContent: ContentData
      switch (fileType) {
        case 'markdown': {
          const text = await readFile(filePath)
          newContent = { type: 'markdown', text }
          break
        }
        case 'text': {
          const text = await readFile(filePath)
          newContent = { type: 'text', text }
          break
        }
        case 'code': {
          const text = await readFile(filePath)
          newContent = { type: 'code', text, language: getLanguage(fileName) || 'plaintext' }
          break
        }
        case 'html': {
          const text = await readFile(filePath)
          newContent = { type: 'html', text }
          break
        }
        case 'image': {
          const src = convertFileSrc(filePath)
          newContent = { type: 'image', src }
          break
        }
        case 'pdf': {
          const bytes = await readFileBinary(filePath)
          const data = new Uint8Array(bytes)
          newContent = { type: 'pdf', src: '', data }
          break
        }
        case 'docx': {
          const bytes = await readFileBinary(filePath)
          const mammoth = await import('mammoth')
          const result = await mammoth.default.convertToHtml(
            { arrayBuffer: new Uint8Array(bytes).buffer }
          )
          newContent = { type: 'docx', html: result.value }
          break
        }
        case 'xlsx': {
          const bytes = await readFileBinary(filePath)
          const XLSX = await import('xlsx')
          const wb = XLSX.read(new Uint8Array(bytes), { type: 'array' })
          const sheets = wb.SheetNames.map(name => {
            const ws = wb.Sheets[name]
            const rows: string[][] = XLSX.utils.sheet_to_json(ws, { header: 1, defval: '' })
            return { name, rows }
          })
          newContent = { type: 'xlsx', sheets }
          break
        }
        default: {
          // Try opening unknown/extensionless files as text
          try {
            const text = await readFile(filePath)
            newContent = { type: 'text', text }
          } catch {
            showToast(t('toast.unsupported'))
            setLoading(false)
            return
          }
          break
        }
      }

      if (reqId !== openRequestRef.current) return
      setContent(newContent)
      if ('text' in newContent) setLatestText(newContent.text)
      setActiveSnapshotId(null)
      setEditing(false)
      setHasUnsavedChanges(false)
      setIsReadOnly(false)
      setCurrentFile(fileName)
      // Only change currentDir if file is outside current workspace (tree browsing preserves workspace root)
      setCurrentDir(prev => {
        if (!prev) return parts.slice(0, -1).join('/')
        return filePath.startsWith(prev + '/') ? prev : parts.slice(0, -1).join('/')
      })
      setCurrentFilePath(filePath)

      // Snapshots only for text-based files
      if (isTextBased(fileType) && 'text' in newContent) {
        await createSnapshot(filePath, newContent.text).catch(() => {})
        await refreshSnapshots(filePath)
      } else {
        setSnapshots([])
      }
    } catch (err) {
      if (reqId !== openRequestRef.current) return
      showToast(t('toast.openFailed'), 4000)
    } finally {
      if (reqId === openRequestRef.current) setLoading(false)
    }
  }, [showToast, refreshSnapshots, hasUnsavedChanges, currentFilePath, currentFile, content, t])

  // Keep ref in sync for event listeners
  openFileRef.current = openFile
  handleSaveRef.current = handleSave
  toggleDevModeRef.current = toggleDevMode

  const handleSelectFile = useCallback((absolutePath: string) => {
    openFile(absolutePath)
  }, [openFile])

  const handleNavigateDir = useCallback(async (path: string) => {
    setCurrentDir(path)
    setSidebarOpen(true)
    ragInit(path).catch(e => console.warn('RAG init failed:', e))
    preloadPythonEnv().catch(e => console.warn('Python preload failed:', e))
  }, [])

  const handleSelectSnapshot = useCallback(async (id: number | null) => {
    if (id === null) {
      setActiveSnapshotId(null)
      // Restore latest text to current content type
      const ft = currentFile ? getFileType(currentFile) : 'markdown'
      if (ft === 'markdown') setContent({ type: 'markdown', text: latestText })
      else if (ft === 'code') setContent({ type: 'code', text: latestText, language: getLanguage(currentFile) || 'plaintext' })
      else if (ft === 'html') setContent({ type: 'html', text: latestText })
      else setContent({ type: 'text', text: latestText })
      return
    }
    try {
      const text = await getSnapshotContent(id)
      setActiveSnapshotId(id)
      const ft = currentFile ? getFileType(currentFile) : 'markdown'
      if (ft === 'markdown') setContent({ type: 'markdown', text })
      else if (ft === 'code') setContent({ type: 'code', text, language: getLanguage(currentFile) || 'plaintext' })
      else if (ft === 'html') setContent({ type: 'html', text })
      else setContent({ type: 'text', text })
      setEditing(false)
    } catch {
      showToast(t('toast.loadHistoryFailed'))
    }
  }, [latestText, currentFile, showToast, t])

  const handleEdit = useCallback((text: string) => {
    const ft = currentFile ? getFileType(currentFile) : 'markdown'
    if (ft === 'markdown') setContent({ type: 'markdown', text })
    else if (ft === 'code') setContent({ type: 'code', text, language: getLanguage(currentFile) || 'plaintext' })
    else if (ft === 'html') setContent({ type: 'html', text })
    else setContent({ type: 'text', text })
    setHasUnsavedChanges(true)
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    if (currentFilePath) {
      saveTimerRef.current = setTimeout(async () => {
        try {
          await saveFile(currentFilePath, text)
          setLatestText(text)
          setHasUnsavedChanges(false)
          if (isTextBased(ft)) {
            await createSnapshot(currentFilePath, text).catch(() => {})
            await refreshSnapshots(currentFilePath)
          }
        } catch {
          // Auto-save failed — keep hasUnsavedChanges true so user knows data is not saved
          console.error('[auto-save] failed for', currentFilePath)
        }
      }, AUTOSAVE_DELAY)
    }
  }, [currentFilePath, currentFile, refreshSnapshots])

  const toggleEdit = useCallback(() => {
    if (isViewingHistory || !canEdit) return
    if (!currentFilePath && !editing) {
      showToast(t('toast.openFirst'))
      return
    }
    setEditing(v => !v)
  }, [isViewingHistory, canEdit, currentFilePath, editing, showToast, t])

  const handleOpen = useCallback(async () => {
    const selected = await openFileOrDirDialog()
    if (!selected) return
    try {
      await listDirectory(selected)
      // If listDirectory succeeds, it's a directory
      setCurrentDir(selected)
      setSidebarOpen(true)
      // Initialize RAG indexing in background
      ragInit(selected).catch(e => console.warn('RAG init failed:', e))
      preloadPythonEnv().catch(e => console.warn('Python preload failed:', e))
    } catch {
      // Not a directory, treat as file
      openFile(selected)
    }
  }, [openFile])

  // Keep refs in sync (declared after all callbacks)
  handleOpenRef.current = handleOpen
  toggleEditRef.current = toggleEdit

  // Drag and drop with visual feedback
  useEffect(() => {
    const handleDragEnter = (e: DragEvent) => {
      e.preventDefault()
      dragCountRef.current++
      if (dragCountRef.current === 1) setDragging(true)
    }
    const handleDragLeave = (e: DragEvent) => {
      e.preventDefault()
      dragCountRef.current--
      if (dragCountRef.current === 0) setDragging(false)
    }
    const handleDragOver = (e: DragEvent) => { e.preventDefault(); e.stopPropagation() }
    const handleDrop = async (e: DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      dragCountRef.current = 0
      setDragging(false)
      const droppedFiles = e.dataTransfer?.files
      if (droppedFiles && droppedFiles.length > 0) {
        const file = droppedFiles[0]
        const ft = getFileType(file.name)
        try {
          if (isTextBased(ft)) {
            const text = await file.text()
            let newContent: ContentData
            if (ft === 'markdown') newContent = { type: 'markdown', text }
            else if (ft === 'code') newContent = { type: 'code', text, language: getLanguage(file.name) || 'plaintext' }
            else if (ft === 'html') newContent = { type: 'html', text }
            else newContent = { type: 'text', text }
            setContent(newContent)
            setLatestText(text)
          } else if (ft === 'image') {
              const url = URL.createObjectURL(file)
              // Revoke previous blob URL if any
              if (content.type === 'image' && content.src.startsWith('blob:')) {
                URL.revokeObjectURL(content.src)
              }
              setContent({ type: 'image', src: url })
            } else if (ft === 'pdf') {
              const buf = await file.arrayBuffer()
              setContent({ type: 'pdf', src: '', data: new Uint8Array(buf) })
            } else if (ft === 'docx') {
              const mammoth = await import('mammoth')
              const buf = await file.arrayBuffer()
              const result = await mammoth.default.convertToHtml({ arrayBuffer: buf })
              setContent({ type: 'docx', html: result.value })
            } else if (ft === 'xlsx') {
              const XLSX = await import('xlsx')
              const buf = await file.arrayBuffer()
              const wb = XLSX.read(new Uint8Array(buf), { type: 'array' })
              const sheets = wb.SheetNames.map(name => {
                const ws = wb.Sheets[name]
                const rows: string[][] = XLSX.utils.sheet_to_json(ws, { header: 1, defval: '' })
                return { name, rows }
              })
              setContent({ type: 'xlsx', sheets })
            } else {
              // Unknown type — try as text
              const text = await file.text()
              setContent({ type: 'text', text })
              setLatestText(text)
            }
            setCurrentFile(file.name)
            setActiveSnapshotId(null)
            setEditing(false)
            setHasUnsavedChanges(false)
            setCurrentFilePath('')
            setIsReadOnly(true)
            setSnapshots([])
            showToast(t('toast.opened', { name: file.name }))
          } catch {
            showToast(t('toast.readFailed'))
          }
      }
    }
    document.addEventListener('dragenter', handleDragEnter)
    document.addEventListener('dragleave', handleDragLeave)
    document.addEventListener('dragover', handleDragOver)
    document.addEventListener('drop', handleDrop)
    return () => {
      document.removeEventListener('dragenter', handleDragEnter)
      document.removeEventListener('dragleave', handleDragLeave)
      document.removeEventListener('dragover', handleDragOver)
      document.removeEventListener('drop', handleDrop)
    }
  }, [showToast, t])

  // Clean up timers and blob URLs on unmount
  useEffect(() => {
    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current)
      if (prevBlobUrlRef.current) URL.revokeObjectURL(prevBlobUrlRef.current)
    }
  }, [])

  // Revoke previous blob URL when content changes to avoid memory leaks
  useEffect(() => {
    const currentBlob = content.type === 'image' && content.src.startsWith('blob:') ? content.src : null
    if (prevBlobUrlRef.current && prevBlobUrlRef.current !== currentBlob) {
      URL.revokeObjectURL(prevBlobUrlRef.current)
    }
    prevBlobUrlRef.current = currentBlob
  }, [content])

  // File association: initial file on launch + runtime events
  useEffect(() => {
    getInitialFile().then(async (path) => {
      if (!path) return
      // Check if it's a directory by trying listDirectory
      try {
        await listDirectory(path)
        setCurrentDir(path)
        setSidebarOpen(true)
        ragInit(path).catch(e => console.warn('RAG init failed:', e))
        preloadPythonEnv().catch(e => console.warn('Python preload failed:', e))
      } catch {
        openFileRef.current(path)
      }
    })
    let unlisten: (() => void) | undefined
    listen<string>('file-open', (event) => {
      openFileRef.current(event.payload)
    }).then(fn => { unlisten = fn })
    // Auto-update check (silent, non-blocking)
    import('@tauri-apps/plugin-updater').then(({ check }) => {
      check().then(async (update) => {
        if (update) {
          const yes = window.confirm(t('update.available', { version: update.version }))
          if (yes) {
            await update.downloadAndInstall()
            const { relaunch } = await import('@tauri-apps/plugin-process')
            await relaunch()
          }
        }
      }).catch(() => { /* silent — no update or offline */ })
    }).catch(() => {})
    return () => { unlisten?.() }
  }, [])

  // Handle native menu actions
  useEffect(() => {
    let unlistenMenu: (() => void) | undefined
    listen<string>('menu-action', (event) => {
      switch (event.payload) {
        case 'open': handleOpenRef.current(); break
        case 'save': handleSaveRef.current(); break
        case 'find': setFindBarOpen(v => !v); break
        case 'toggle_edit': toggleEditRef.current(); break
        case 'dev_mode': toggleDevModeRef.current(); break
        case 'settings': setSettingsOpen(true); break
        case 'shortcuts': setShowAbout(true); break
      }
    }).then(fn => { unlistenMenu = fn })
    return () => { unlistenMenu?.() }
  }, [])

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey
      const key = e.key.toLowerCase()
      if (mod && key === 'o') {
        e.preventDefault()
        handleOpen()
      } else if (mod && key === 'e' && !e.shiftKey) {
        e.preventDefault()
        toggleEdit()
      } else if (mod && key === 'f') {
        e.preventDefault()
        setFindBarOpen(v => !v)
      } else if (mod && e.key === 's') {
        e.preventDefault()
        handleSave()
      } else if (mod && e.shiftKey && key === 'e') {
        e.preventDefault()
        if (canExport) {
          const text = getContentText(content)
          setLoading(true)
          exportFile('PDF', text, themeId, currentFilePath || currentFile || 'document', isPro)
            .then(msg => showToast(msg))
            .catch(() => showToast(t('toast.exportFailed')))
            .finally(() => setLoading(false))
        } else {
          showToast(t('toast.exportNotSupported'))
        }
      } else if (e.key === 'Escape' && editing) {
        setEditing(false)
      } else if (mod && key === 'd') {
        e.preventDefault()
        toggleDevMode()
      } else if (mod && e.key === '`') {
        e.preventDefault()
        if (devMode) setTerminalVisible(v => !v)
      } else if (mod && e.shiftKey && key === 'l') {
        e.preventDefault()
        if (devMode) setDevConsoleOpen(v => !v)
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [toggleEdit, handleSave, handleOpen, editing, content, themeId, currentFile, showToast, canExport, toggleDevMode, devMode])

  return (
    <div className="flex flex-col h-full">
      <Toolbar
        themeId={themeId}
        onToggleSidebar={() => setSidebarOpen(v => !v)}
        currentFile={currentFile}
        currentDir={currentDir}
        currentFilePath={currentFilePath}
        onExport={async (fmt) => {
          if (!canExport) {
            showToast(t('toast.exportNotSupported'))
            return
          }
          setLoading(true)
          try {
            const text = getContentText(content)
            const msg = await exportFile(fmt, text, themeId, currentFilePath || currentFile || 'document', isPro)
            showToast(msg)
          } catch {
            showToast(t('toast.exportFailed'))
          } finally {
            setLoading(false)
          }
        }}
        onNavigateDir={handleNavigateDir}
        isViewingHistory={isViewingHistory}
        onBackToLatest={() => handleSelectSnapshot(null)}
        isEditing={editing}
        onToggleEdit={toggleEdit}
        hasUnsavedChanges={hasUnsavedChanges}
        onOpenFile={handleOpen}
        onOpenFilePath={openFile}
        isReadOnly={isReadOnly || !canEdit}
        loading={loading}
        devMode={devMode}
        onToggleDevMode={() => {
          if (!isPro) { setLicensePanelOpen(true); return }
          toggleDevMode()
        }}
        onToggleAI={() => setAiPanelOpen(v => !v)}
        aiPanelOpen={aiPanelOpen}
        onOpenSettings={() => setSettingsOpen(true)}
        isPro={isPro}
        onOpenLicense={() => setLicensePanelOpen(true)}
      />
      <div className="flex flex-1 overflow-hidden" style={{ paddingTop: 52 }}>
        {!currentFilePath && !currentDir ? (
          <WelcomeScreen recentDirs={recentDirs} onOpenDir={handleNavigateDir} onOpen={handleOpen} />
        ) : (
        <>
        <div className="flex flex-col overflow-hidden" style={{ width: sidebarOpen ? 220 : 0, minWidth: sidebarOpen ? 220 : 0, transition: 'width 250ms ease-in-out, min-width 250ms ease-in-out', background: 'var(--sidebar-bg)', borderRight: sidebarOpen ? '1px solid var(--border-s)' : 'none' }}>
          <Sidebar
            open={sidebarOpen}
            currentFile={currentFile}
            currentFilePath={currentFilePath}
            files={files}
            currentDir={currentDir}
            onSelectFile={handleSelectFile}
            onNewFile={() => currentDir && setNewFilePopup('file')}
            onNewFolder={() => currentDir && setNewFilePopup('folder')}
            onRename={handleRenameEntry}
            onDelete={handleDeleteEntry}
            onCopyPath={handleCopyPath}
            devMode={devMode}
            isGitRepo={isGitRepo}
            gitBranch={gitBranch}
            gitChangedCount={gitChangedCount}
            onGitInit={handleGitInit}
            onOpenGitPanel={() => setGitPanelOpen(true)}
            onSwitchDir={(path) => {
              const dirName = path.split('/').pop() || path
              const doSwitch = () => {
                setCurrentDir(path)
                setCurrentFile('')
                setCurrentFilePath('')
                setContent({ type: 'markdown', text: '' })
                showToast(t('toast.workspaceSwitched', { dir: dirName }))
              }
              if (aiPanelOpen && aiBusyRef.current) {
                if (window.confirm(t('toast.switchDirConfirm', { dir: dirName }))) {
                  doSwitch()
                }
              } else {
                doSwitch()
              }
            }}
          />
        </div>
        <div className="flex flex-col flex-1 overflow-hidden relative">
          {loading && (
            <div className="loading-overlay">
              <div className="loading-spinner" />
            </div>
          )}
          <FindBar visible={findBarOpen} onClose={() => setFindBarOpen(false)} />
          <ContentArea
            content={content}
            themeId={themeId}
            editing={editing}
            onEdit={handleEdit}
            currentFilePath={currentFilePath}
            onExport={async (fmt) => {
              if (!canExport) { showToast(t('toast.exportNotSupported')); return }
              setLoading(true)
              try {
                const text = getContentText(content)
                const msg = await exportFile(fmt, text, themeId, currentFilePath || currentFile || 'document', isPro)
                showToast(msg)
              } catch { showToast(t('toast.exportFailed')) }
              finally { setLoading(false) }
            }}
            onToggleEdit={toggleEdit}
            canExport={canExport}
            canEdit={canEdit}
            isReadOnly={isReadOnly}
          />
          {!isHtmlPreview && (canSnapshot ? (
            <Timeline
              open={timelineOpen}
              snapshots={snapshots}
              activeSnapshotId={activeSnapshotId}
              onSelectSnapshot={handleSelectSnapshot}
            />
          ) : timelineOpen ? (
            <div
              className="text-center text-[12px] py-2"
              style={{ color: 'var(--text-3)', borderTop: '1px solid var(--border-s)' }}
            >
              {t('timeline.notSupported')}
            </div>
          ) : null)}
          {devMode && isPro && (
            <TerminalPanel
              cwd={currentDir || homeDir}
              visible={terminalVisible}
              onOpenSettings={() => setSettingsOpen(true)}
              onOpenLog={(logContent) => {
                setContent({ type: 'code', text: logContent, language: 'log' })
                setCurrentFile('terminal-session.log')
                setCurrentFilePath('')
                setEditing(false)
              }}
            />
          )}
        </div>
        </>
        )}
      </div>
      {dragging && (
        <div className="drag-overlay">
          <div className="drag-overlay-text">{t('toast.dragHint')}</div>
        </div>
      )}
      <Toast message={toast} />
      {devMode && <DevConsole visible={devConsoleOpen} onClose={() => setDevConsoleOpen(false)} />}
      {newFilePopup && (
        <NewFilePopup
          type={newFilePopup}
          onConfirm={(name, template) => {
            if (newFilePopup === 'folder') handleCreateFolder(name)
            else handleCreateFile(name, template)
          }}
          onCancel={() => setNewFilePopup(null)}
        />
      )}
      {conflictFile && (
        <ConflictDialog
          fileName={conflictFile}
          onKeepMine={() => {}}
          onAcceptExternal={() => {
            if (currentFilePath) {
              readFile(currentFilePath).then(text => {
                const ft = getFileType(currentFile)
                if (ft === 'markdown') setContent({ type: 'markdown', text })
                else if (ft === 'code') setContent({ type: 'code', text, language: getLanguage(currentFile) || 'plaintext' })
                else if (ft === 'html') setContent({ type: 'html', text })
                else setContent({ type: 'text', text })
                setLatestText(text)
                setHasUnsavedChanges(false)
              }).catch(() => showToast(t('toast.loadExternalFailed')))
            }
          }}
          onDismiss={() => setConflictFile(null)}
        />
      )}
      <GitPanel cwd={currentDir} visible={gitPanelOpen} onToast={showToast} onClose={() => setGitPanelOpen(false)} isPro={isPro} onOpenLicense={() => setLicensePanelOpen(true)} />
      <AIChatPanel visible={aiPanelOpen} currentDir={currentDir} onClose={() => setAiPanelOpen(false)} onToast={showToast} isPro={isPro} onOpenLicense={() => setLicensePanelOpen(true)} busyRef={aiBusyRef} onOpenFile={(path, _line) => {
        // Resolve relative paths against currentDir
        const fullPath = path.startsWith('/') || path.match(/^[A-Z]:\\/) ? path : (currentDir ? currentDir + '/' + path : path)
        handleSelectFile(fullPath)
      }} />
      <SettingsPanel
        visible={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        themeId={themeId}
        onSetTheme={setThemeId}
        onToast={showToast}
        onOpenLicense={() => setLicensePanelOpen(true)}
        currentDir={currentDir}
      />
      <LicensePanel
        visible={licensePanelOpen}
        onClose={() => setLicensePanelOpen(false)}
        onToast={showToast}
      />
      {gitInitConfirm && (
        <div className="shortcuts-backdrop" onClick={() => setGitInitConfirm(null)}>
          <div className="shortcuts-modal" onClick={e => e.stopPropagation()} style={{ minWidth: 320 }}>
            <div className="flex items-center justify-between mb-1">
              <h3 style={{ margin: 0 }}>{t('gitInit.title')}</h3>
              <button className="sidebar-action-btn" onClick={() => setGitInitConfirm(null)} aria-label={t('ai.close')}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
                  <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            </div>
            <p className="text-[13px] mb-3" style={{ color: 'var(--text-2)', lineHeight: '1.6' }}>{t('gitInit.message')}</p>
            <div className="text-[12px] px-4 py-3 mb-5 break-all" style={{ fontFamily: "'JetBrains Mono', monospace", borderRadius: 'var(--radius-sm)', background: 'var(--sidebar-bg)', color: 'var(--text)' }}>
              {gitInitConfirm}
            </div>
            <div className="flex justify-end gap-3">
              <button className="toolbar-btn" onClick={() => setGitInitConfirm(null)}>{t('gitInit.cancel')}</button>
              <button className="toolbar-btn toolbar-btn-accent" onClick={confirmGitInit}>{t('gitInit.confirm')}</button>
            </div>
          </div>
        </div>
      )}
      {showAbout && (
        <div className="shortcuts-backdrop" onClick={() => setShowAbout(false)}>
          <div className="shortcuts-modal" onClick={e => e.stopPropagation()} style={{ minWidth: 380, maxWidth: 440 }}>
            <div className="flex items-center justify-between mb-1">
              <h3 style={{ margin: 0 }}>{t('about.title')}</h3>
              <button className="sidebar-action-btn" onClick={() => setShowAbout(false)} aria-label={t('ai.close')}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
                  <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            </div>
            <div className="text-[13px] mb-4" style={{ color: 'var(--text-2)', lineHeight: '1.8' }}>
              <p style={{ margin: '4px 0' }}>Inkess v{__APP_VERSION__} — {t('about.tagline')}</p>
              <p style={{ margin: '4px 0' }}>
                <span style={{ color: 'var(--text-3)' }}>{t('about.website')}: </span>
                <span style={{ color: 'var(--accent)' }}>inkess.net</span>
              </p>
              <p style={{ margin: '4px 0' }}>
                <span style={{ color: 'var(--text-3)' }}>{t('about.feedback')}: </span>
                <span style={{ color: 'var(--accent)' }}>gezhigang@foxmail.com</span>
              </p>
            </div>
            <div className="text-[12px]" style={{ color: 'var(--text-2)' }}>
              <h4 style={{ margin: '0 0 8px', fontSize: 13, color: 'var(--text)' }}>{t('about.shortcuts')}</h4>
              <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                <tbody>
                  {[
                    ['⌘ O', t('about.shortcut.open')],
                    ['⌘ E', t('about.shortcut.edit')],
                    ['⌘ S', t('about.shortcut.save')],
                    ['⌘ F', t('about.shortcut.find')],
                    ['⌘ ⇧ E', t('about.shortcut.export')],
                    ['⌘ D', t('about.shortcut.devMode')],
                    ['⌘ `', t('about.shortcut.terminal')],
                    ['⌘ ⇧ L', t('about.shortcut.devConsole')],
                    ['Esc', t('about.shortcut.exitEdit')],
                  ].map(([key, desc]) => (
                    <tr key={key} style={{ borderBottom: '1px solid var(--border-s)' }}>
                      <td style={{ padding: '4px 8px 4px 0', fontFamily: "'JetBrains Mono', monospace", whiteSpace: 'nowrap', color: 'var(--text-3)' }}>{key}</td>
                      <td style={{ padding: '4px 0' }}>{desc}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p className="text-[11px] mt-3" style={{ color: 'var(--text-3)', margin: '12px 0 0' }}>© 2025 Inkess. All rights reserved.</p>
          </div>
        </div>
      )}
    </div>
  )
}