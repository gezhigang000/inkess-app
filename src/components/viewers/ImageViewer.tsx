import { useState } from 'react'
import { useI18n } from '../../lib/i18n'

interface ImageViewerProps {
  src: string
}

export function ImageViewer({ src }: ImageViewerProps) {
  const { t } = useI18n()
  const [scale, setScale] = useState(1)

  return (
    <div className="flex-1 overflow-auto flex items-center justify-center p-10">
      <div className="flex flex-col items-center gap-4">
        <img
          src={src}
          alt="Preview"
          className="max-w-full rounded shadow-lg transition-transform"
          style={{ transform: `scale(${scale})`, transformOrigin: 'center' }}
          onWheel={e => {
            e.preventDefault()
            setScale(s => Math.max(0.1, Math.min(5, s + (e.deltaY > 0 ? -0.1 : 0.1))))
          }}
        />
        <div className="flex items-center gap-2 text-[12px]" style={{ color: 'var(--text-3)' }}>
          <button
            className="px-2 py-1 rounded cursor-pointer border-none"
            style={{ background: 'var(--sidebar-bg)', color: 'var(--text-2)' }}
            onClick={() => setScale(s => Math.max(0.1, s - 0.25))}
          >
            -
          </button>
          <span>{Math.round(scale * 100)}%</span>
          <button
            className="px-2 py-1 rounded cursor-pointer border-none"
            style={{ background: 'var(--sidebar-bg)', color: 'var(--text-2)' }}
            onClick={() => setScale(s => Math.min(5, s + 0.25))}
          >
            +
          </button>
          <button
            className="px-2 py-1 rounded cursor-pointer border-none"
            style={{ background: 'var(--sidebar-bg)', color: 'var(--text-2)' }}
            onClick={() => setScale(1)}
          >
            {t('viewer.reset')}
          </button>
        </div>
      </div>
    </div>
  )
}
