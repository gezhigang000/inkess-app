import { useState, useEffect, useRef, useCallback } from 'react'
import { getDebugLogs, clearDebugLogs, type DebugLogEntry } from '../lib/tauri'

interface DevConsoleProps {
  visible: boolean
  onClose: () => void
}

const LEVEL_COLORS: Record<string, string> = {
  error: '#ef4444',
  warn: '#f59e0b',
  info: '#6b7280',
}

export function DevConsole({ visible, onClose }: DevConsoleProps) {
  const [logs, setLogs] = useState<DebugLogEntry[]>([])
  const [filter, setFilter] = useState('')
  const [autoScroll, setAutoScroll] = useState(true)
  const bottomRef = useRef<HTMLDivElement>(null)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetchLogs = useCallback(() => {
    getDebugLogs().then(setLogs).catch(() => {})
  }, [])

  useEffect(() => {
    if (!visible) return
    fetchLogs()
    intervalRef.current = setInterval(fetchLogs, 2000)
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current)
    }
  }, [visible, fetchLogs])

  useEffect(() => {
    if (autoScroll && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [logs, autoScroll])

  const handleClear = () => {
    clearDebugLogs().then(() => setLogs([])).catch(() => {})
  }

  const filtered = filter
    ? logs.filter(l =>
        l.message.toLowerCase().includes(filter.toLowerCase()) ||
        l.module.toLowerCase().includes(filter.toLowerCase()) ||
        l.level.includes(filter.toLowerCase())
      )
    : logs

  if (!visible) return null

  return (
    <div style={{
      position: 'fixed', bottom: 0, left: 0, right: 0, height: 240, zIndex: 100,
      background: '#1a1a2e', color: '#e0e0e0', fontFamily: 'monospace', fontSize: 11,
      borderTop: '2px solid #f59e0b', display: 'flex', flexDirection: 'column',
    }}>
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8, padding: '4px 10px',
        background: '#16213e', borderBottom: '1px solid #333',
      }}>
        <span style={{ fontWeight: 600, color: '#f59e0b' }}>Dev Console</span>
        <input
          value={filter}
          onChange={e => setFilter(e.target.value)}
          placeholder="Filter..."
          style={{
            flex: 1, padding: '2px 6px', fontSize: 11, background: '#0f3460',
            border: '1px solid #444', borderRadius: 3, color: '#e0e0e0', outline: 'none',
          }}
        />
        <label style={{ fontSize: 10, cursor: 'pointer', userSelect: 'none' }}>
          <input type="checkbox" checked={autoScroll} onChange={e => setAutoScroll(e.target.checked)} style={{ marginRight: 3 }} />
          Auto-scroll
        </label>
        <button onClick={handleClear} style={{
          padding: '2px 8px', fontSize: 10, background: '#333', color: '#ccc',
          border: '1px solid #555', borderRadius: 3, cursor: 'pointer',
        }}>Clear</button>
        <button onClick={onClose} style={{
          padding: '2px 8px', fontSize: 10, background: '#333', color: '#ccc',
          border: '1px solid #555', borderRadius: 3, cursor: 'pointer',
        }}>×</button>
      </div>
      <div style={{ flex: 1, overflowY: 'auto', padding: '4px 10px' }}>
        {filtered.length === 0 && (
          <div style={{ color: '#666', padding: 8 }}>No log entries yet. Logs will appear as you use the app.</div>
        )}
        {filtered.map((entry, i) => (
          <div key={i} style={{ display: 'flex', gap: 8, lineHeight: '18px', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
            <span style={{ color: '#666', flexShrink: 0 }}>{entry.timestamp}</span>
            <span style={{ color: LEVEL_COLORS[entry.level] || '#6b7280', flexShrink: 0, width: 36, textAlign: 'right' }}>{entry.level}</span>
            <span style={{ color: '#60a5fa', flexShrink: 0 }}>[{entry.module}]</span>
            <span>{entry.message}</span>
          </div>
        ))}
        <div ref={bottomRef} />
      </div>
    </div>
  )
}
