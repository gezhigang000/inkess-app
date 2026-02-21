import { useState } from 'react'
import type { SheetData } from '../../types/content'
import { useI18n } from '../../lib/i18n'

function colLabel(ci: number): string {
  let label = ''
  let n = ci
  while (n >= 0) {
    label = String.fromCharCode(65 + (n % 26)) + label
    n = Math.floor(n / 26) - 1
  }
  return label
}

interface ExcelViewerProps {
  sheets: SheetData[]
}

export function ExcelViewer({ sheets }: ExcelViewerProps) {
  const { t } = useI18n()
  const [activeSheet, setActiveSheet] = useState(0)
  const sheet = sheets[activeSheet]

  if (!sheets.length) {
    return <div className="flex items-center justify-center h-full" style={{ color: 'var(--text-3)' }}>{t('viewer.emptyWorkbook')}</div>
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {sheets.length > 1 && (
        <div className="flex gap-0 px-2 pt-2 shrink-0" style={{ borderBottom: '1px solid var(--border-s)' }}>
          {sheets.map((s, i) => (
            <button
              key={i}
              onClick={() => setActiveSheet(i)}
              className="px-3 py-1.5 text-[12px] border-none cursor-pointer"
              style={{
                background: i === activeSheet ? 'var(--bg-1)' : 'transparent',
                color: i === activeSheet ? 'var(--text-1)' : 'var(--text-3)',
                borderBottom: i === activeSheet ? '2px solid var(--color-accent)' : '2px solid transparent',
              }}
            >
              {s.name}
            </button>
          ))}
        </div>
      )}
      <div className="flex-1 overflow-auto">
        <table className="excel-table">
          <thead>
            {sheet.rows.length > 0 && (
              <tr>
                <th className="excel-row-num">#</th>
                {sheet.rows[0].map((_, ci) => (
                  <th key={ci}>{colLabel(ci)}</th>
                ))}
              </tr>
            )}
          </thead>
          <tbody>
            {sheet.rows.map((row, ri) => (
              <tr key={ri}>
                <td className="excel-row-num">{ri + 1}</td>
                {row.map((cell, ci) => (
                  <td key={ci}>{cell}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
