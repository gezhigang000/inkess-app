import { useState, useRef, useEffect, useCallback } from 'react'
import { searchFiles } from '../lib/tauri'
import { useI18n } from '../lib/i18n'

interface SearchBarProps {
  currentDir: string
  onOpenFile: (path: string) => void
  onOpenDir: (path: string) => void
}

export function SearchBar({ currentDir, onOpenFile, onOpenDir }: SearchBarProps) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<string[]>([])
  const [selected, setSelected] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined)
  const { t } = useI18n()

  const doSearch = useCallback(async (q: string) => {
    if (!currentDir || !q.trim()) {
      setResults([])
      return
    }
    try {
      const res = await searchFiles(currentDir, q.trim())
      setResults(res)
      setSelected(0)
    } catch {
      setResults([])
    }
  }, [currentDir])

  useEffect(() => {
    return () => clearTimeout(timerRef.current)
  }, [])

  useEffect(() => {
    if (!open) return
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [open])

  const handleChange = (val: string) => {
    setQuery(val)
    clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => doSearch(val), 200)
  }

  const handleSelect = (relPath: string) => {
    if (relPath.includes('..')) return
    const fullPath = currentDir + '/' + relPath
    setOpen(false)
    setQuery('')
    setResults([])
    // Check if it looks like a directory (no extension or ends with /)
    if (relPath.endsWith('/') || !relPath.split('/').pop()?.includes('.')) {
      onOpenDir(fullPath)
    } else {
      onOpenFile(fullPath)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelected(s => Math.min(s + 1, results.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelected(s => Math.max(s - 1, 0))
    } else if (e.key === 'Enter' && results[selected]) {
      handleSelect(results[selected])
    } else if (e.key === 'Escape') {
      setOpen(false)
    }
  }

  if (!currentDir) return null

  return (
    <div className="relative" ref={panelRef}>
      <button
        className="toolbar-btn"
        title={currentDir ? t('search.title') : t('toolbar.needWorkspace')}
        aria-label={t('search.title')}
        disabled={!currentDir}
        style={!currentDir ? { opacity: 0.35, cursor: 'not-allowed' } : undefined}
        onClick={() => { if (!currentDir) return; setOpen(v => !v); setTimeout(() => inputRef.current?.focus(), 50) }}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
          <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
      </button>
      {open && (
        <div
          className="absolute top-[calc(100%+6px)] right-0 w-[320px] rounded-[10px] overflow-hidden z-50"
          style={{ background: 'var(--surface)', border: '1px solid var(--border)', boxShadow: 'var(--shadow-lg)' }}
        >
          <div className="px-3 py-2" style={{ borderBottom: '1px solid var(--border-s)' }}>
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={e => handleChange(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t('search.placeholder')}
              className="w-full text-[13px] px-2.5 py-1.5 rounded-md border-none outline-none"
              style={{ background: 'var(--sidebar-bg)', color: 'var(--text)', fontFamily: 'inherit' }}
            />
          </div>
          <div className="max-h-[300px] overflow-auto">
            {results.length === 0 && query.trim() && (
              <div className="px-4 py-3 text-[12px]" style={{ color: 'var(--text-3)' }}>{t('search.noResults')}</div>
            )}
            {results.map((r, i) => {
              const parts = r.split('/')
              const name = parts.pop() || r
              const dir = parts.join('/')
              return (
                <button
                  key={r}
                  className="w-full flex items-center gap-2 px-3 py-2 text-left cursor-pointer border-none transition-colors"
                  style={{
                    background: i === selected ? 'var(--accent-subtle)' : 'transparent',
                    color: 'var(--text)',
                    fontFamily: 'inherit',
                  }}
                  onMouseEnter={() => setSelected(i)}
                  onClick={() => handleSelect(r)}
                >
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="w-3.5 h-3.5 shrink-0" style={{ color: 'var(--text-3)' }}>
                    {name.includes('.') ? (
                      <><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /></>
                    ) : (
                      <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
                    )}
                  </svg>
                  <div className="min-w-0 flex-1">
                    <div className="text-[13px] truncate">{name}</div>
                    {dir && <div className="text-[11px] truncate" style={{ color: 'var(--text-3)' }}>{dir}</div>}
                  </div>
                </button>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}
