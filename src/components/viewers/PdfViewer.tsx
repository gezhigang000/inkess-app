import { useState, useEffect, useRef, useCallback } from 'react'
import * as pdfjsLib from 'pdfjs-dist'
import { useI18n } from '../../lib/i18n'

pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
  'pdfjs-dist/build/pdf.worker.mjs',
  import.meta.url,
).toString()

interface PdfViewerProps {
  src: string
  data?: Uint8Array
}

export function PdfViewer({ src, data }: PdfViewerProps) {
  const { t } = useI18n()
  const [numPages, setNumPages] = useState(0)
  const [currentPage, setCurrentPage] = useState(1)
  const [scale, setScale] = useState(1.5)
  const [error, setError] = useState('')
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const pdfDocRef = useRef<pdfjsLib.PDFDocumentProxy | null>(null)

  useEffect(() => {
    let cancelled = false
    setError('')
    const loadPdf = async () => {
      try {
        // Destroy previous document to free memory
        if (pdfDocRef.current) {
          pdfDocRef.current.destroy()
          pdfDocRef.current = null
        }
        const source = data ? { data } : src
        const doc = await pdfjsLib.getDocument(source).promise
        if (cancelled) {
          doc.destroy()
          return
        }
        pdfDocRef.current = doc
        setNumPages(doc.numPages)
        setCurrentPage(1)
      } catch {
        if (!cancelled) setError(t('viewer.pdfLoadFailed'))
      }
    }
    loadPdf()
    return () => {
      cancelled = true
      if (pdfDocRef.current) {
        pdfDocRef.current.destroy()
        pdfDocRef.current = null
      }
    }
  }, [src, data])

  const renderPage = useCallback(async (pageNum: number) => {
    const doc = pdfDocRef.current
    const canvas = canvasRef.current
    if (!doc || !canvas) return
    try {
      const page = await doc.getPage(pageNum)
      const viewport = page.getViewport({ scale })
      canvas.width = viewport.width
      canvas.height = viewport.height
      const ctx = canvas.getContext('2d')
      if (!ctx) return
      await page.render({ canvasContext: ctx, viewport, canvas } as any).promise
    } catch {
      setError(t('viewer.renderFailed'))
    }
  }, [scale])

  useEffect(() => {
    if (numPages > 0) renderPage(currentPage)
  }, [currentPage, numPages, renderPage])

  if (error) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ color: 'var(--text-3)' }}>
        {error}
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-auto flex flex-col items-center p-6 gap-4">
      <canvas ref={canvasRef} className="shadow-lg rounded" />
      {numPages > 0 && (
        <div className="flex items-center gap-3 text-[13px]" style={{ color: 'var(--text-2)' }}>
          <button
            className="px-2 py-1 rounded cursor-pointer border-none disabled:opacity-30"
            style={{ background: 'var(--sidebar-bg)', color: 'var(--text-2)' }}
            disabled={currentPage <= 1}
            onClick={() => setCurrentPage(p => p - 1)}
          >
            {t('viewer.prevPage')}
          </button>
          <span>{currentPage} / {numPages}</span>
          <button
            className="px-2 py-1 rounded cursor-pointer border-none disabled:opacity-30"
            style={{ background: 'var(--sidebar-bg)', color: 'var(--text-2)' }}
            disabled={currentPage >= numPages}
            onClick={() => setCurrentPage(p => p + 1)}
          >
            {t('viewer.nextPage')}
          </button>
          <span className="mx-2">|</span>
          <button
            className="px-2 py-1 rounded cursor-pointer border-none"
            style={{ background: 'var(--sidebar-bg)', color: 'var(--text-2)' }}
            onClick={() => setScale(s => Math.max(0.5, s - 0.25))}
          >
            -
          </button>
          <span>{Math.round(scale * 100)}%</span>
          <button
            className="px-2 py-1 rounded cursor-pointer border-none"
            style={{ background: 'var(--sidebar-bg)', color: 'var(--text-2)' }}
            onClick={() => setScale(s => Math.min(4, s + 0.25))}
          >
            +
          </button>
        </div>
      )}
    </div>
  )
}
