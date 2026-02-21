import { useState, useCallback, useRef, useEffect } from 'react'
import { useI18n } from '../lib/i18n'

interface TableBuilderDialogProps {
  onInsert: (rows: number, cols: number) => void
  onClose: () => void
}

const MAX_COLS = 8
const MAX_ROWS = 6

export function TableBuilderDialog({ onInsert, onClose }: TableBuilderDialogProps) {
  const { t } = useI18n()
  const [hoverCol, setHoverCol] = useState(0)
  const [hoverRow, setHoverRow] = useState(0)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose()
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [onClose])

  const handleClick = useCallback(() => {
    if (hoverCol > 0 && hoverRow > 0 && hoverCol <= MAX_COLS && hoverRow <= MAX_ROWS) {
      onInsert(hoverRow, hoverCol)
    }
  }, [hoverCol, hoverRow, onInsert])

  return (
    <div ref={ref} className="table-builder">
      <div
        className="table-builder-grid"
        style={{ gridTemplateColumns: `repeat(${MAX_COLS}, 18px)` }}
      >
        {Array.from({ length: MAX_ROWS * MAX_COLS }, (_, i) => {
          const row = Math.floor(i / MAX_COLS) + 1
          const col = (i % MAX_COLS) + 1
          const active = col <= hoverCol && row <= hoverRow
          return (
            <button
              key={i}
              className={`table-builder-cell${active ? ' active' : ''}`}
              onMouseEnter={() => { setHoverCol(col); setHoverRow(row) }}
              onClick={handleClick}
            />
          )
        })}
      </div>
      {hoverCol > 0 && hoverRow > 0 && (
        <div className="table-builder-label">
          {t('mdToolbar.tableSize', { cols: String(hoverCol), rows: String(hoverRow) })}
        </div>
      )}
    </div>
  )
}
