import { useState, useRef, useEffect, useCallback } from 'react'
import { useI18n } from '../lib/i18n'

interface FindBarProps {
  visible: boolean
  onClose: () => void
}

export function FindBar({ visible, onClose }: FindBarProps) {
  const { t } = useI18n()
  const [query, setQuery] = useState('')
  const [currentIdx, setCurrentIdx] = useState(0)
  const [totalMatches, setTotalMatches] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const highlightsRef = useRef<HTMLElement[]>([])

  useEffect(() => {
    if (visible && inputRef.current) {
      inputRef.current.focus()
      inputRef.current.select()
    }
    if (!visible) {
      clearHighlights()
      setQuery('')
      setCurrentIdx(0)
      setTotalMatches(0)
    }
  }, [visible])

  const clearHighlights = useCallback(() => {
    // Remove all existing highlights
    const marks = document.querySelectorAll('mark[data-find-highlight]')
    marks.forEach(mark => {
      const parent = mark.parentNode
      if (parent) {
        parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
        parent.normalize()
      }
    })
    highlightsRef.current = []
  }, [])

  const doSearch = useCallback((searchQuery: string) => {
    clearHighlights()
    if (!searchQuery.trim()) {
      setTotalMatches(0)
      setCurrentIdx(0)
      return
    }

    // Search in the content area
    const contentEl = document.querySelector('.md-body, .flex-1.overflow-y-auto')
    if (!contentEl) { setTotalMatches(0); return }

    const walker = document.createTreeWalker(contentEl, NodeFilter.SHOW_TEXT)
    const textNodes: Text[] = []
    let node: Text | null
    while ((node = walker.nextNode() as Text | null)) {
      textNodes.push(node)
    }

    const lowerQuery = searchQuery.toLowerCase()
    const matches: HTMLElement[] = []

    for (const textNode of textNodes) {
      const text = textNode.textContent || ''
      const lowerText = text.toLowerCase()
      let startIdx = 0
      const indices: number[] = []

      while (true) {
        const idx = lowerText.indexOf(lowerQuery, startIdx)
        if (idx === -1) break
        indices.push(idx)
        startIdx = idx + 1
      }

      if (indices.length === 0) continue

      // Split text node and wrap matches
      const parent = textNode.parentNode
      if (!parent) continue

      const frag = document.createDocumentFragment()
      let lastEnd = 0

      for (const idx of indices) {
        if (idx > lastEnd) {
          frag.appendChild(document.createTextNode(text.slice(lastEnd, idx)))
        }
        const mark = document.createElement('mark')
        mark.setAttribute('data-find-highlight', 'true')
        mark.style.cssText = 'background:#fbbf24;color:#1c1917;border-radius:2px;padding:0 1px;'
        mark.textContent = text.slice(idx, idx + searchQuery.length)
        frag.appendChild(mark)
        matches.push(mark)
        lastEnd = idx + searchQuery.length
      }

      if (lastEnd < text.length) {
        frag.appendChild(document.createTextNode(text.slice(lastEnd)))
      }

      parent.replaceChild(frag, textNode)
    }

    highlightsRef.current = matches
    setTotalMatches(matches.length)
    if (matches.length > 0) {
      setCurrentIdx(0)
      scrollToMatch(matches, 0)
    } else {
      setCurrentIdx(0)
    }
  }, [clearHighlights])

  const scrollToMatch = (matches: HTMLElement[], idx: number) => {
    // Reset all highlights to default color
    matches.forEach(m => { m.style.background = '#fbbf24' })
    // Highlight current match
    if (matches[idx]) {
      matches[idx].style.background = '#f97316'
      matches[idx].scrollIntoView({ behavior: 'smooth', block: 'center' })
    }
  }

  const goNext = useCallback(() => {
    if (totalMatches === 0) return
    const next = (currentIdx + 1) % totalMatches
    setCurrentIdx(next)
    scrollToMatch(highlightsRef.current, next)
  }, [currentIdx, totalMatches])

  const goPrev = useCallback(() => {
    if (totalMatches === 0) return
    const prev = (currentIdx - 1 + totalMatches) % totalMatches
    setCurrentIdx(prev)
    scrollToMatch(highlightsRef.current, prev)
  }, [currentIdx, totalMatches])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') { onClose(); return }
    if (e.key === 'Enter' && e.shiftKey) { goPrev(); return }
    if (e.key === 'Enter') { goNext(); return }
  }

  if (!visible) return null

  return (
    <div
      style={{
        position: 'absolute', top: 0, right: 20, zIndex: 60,
        display: 'flex', alignItems: 'center', gap: 6,
        padding: '6px 10px', borderRadius: '0 0 8px 8px',
        background: 'var(--surface)', border: '1px solid var(--border-s)', borderTop: 'none',
        boxShadow: '0 2px 8px rgba(0,0,0,.1)',
      }}
    >
      <input
        ref={inputRef}
        value={query}
        onChange={e => { setQuery(e.target.value); doSearch(e.target.value) }}
        onKeyDown={handleKeyDown}
        placeholder={t('search.inDocPlaceholder')}
        aria-label={t('search.inDoc')}
        style={{
          width: 180, padding: '4px 8px', fontSize: 12, border: '1px solid var(--border-s)',
          borderRadius: 4, background: 'var(--bg)', color: 'var(--text)', outline: 'none',
          fontFamily: 'inherit',
        }}
      />
      <span style={{ fontSize: 11, color: 'var(--text-3)', minWidth: 40, textAlign: 'center' }}>
        {query ? (totalMatches > 0 ? `${currentIdx + 1}/${totalMatches}` : t('search.noMatch')) : ''}
      </span>
      <button onClick={goPrev} className="sidebar-action-btn" title="Previous (Shift+Enter)" aria-label="Previous match" style={{ padding: 3 }}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 12, height: 12 }}><polyline points="18 15 12 9 6 15" /></svg>
      </button>
      <button onClick={goNext} className="sidebar-action-btn" title="Next (Enter)" aria-label="Next match" style={{ padding: 3 }}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 12, height: 12 }}><polyline points="6 9 12 15 18 9" /></svg>
      </button>
      <button onClick={onClose} className="sidebar-action-btn" title="Close (Esc)" aria-label="Close find" style={{ padding: 3 }}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 12, height: 12 }}><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
      </button>
    </div>
  )
}
