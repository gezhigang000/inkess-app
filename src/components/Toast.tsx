export type ToastSeverity = 'info' | 'success' | 'error' | 'warning'

export interface ToastItem {
  id: string
  message: string
  severity: ToastSeverity
}

interface ToastProps {
  toasts: ToastItem[]
  onDismiss: (id: string) => void
}

export function Toast({ toasts, onDismiss }: ToastProps) {
  const visible = toasts.slice(0, 3)
  return (
    <div className="toast-container" role="status" aria-live="polite">
      {visible.map(t => (
        <div key={t.id} className={`toast-item toast-${t.severity}`}>
          <span className="toast-msg">{t.message}</span>
          {t.severity === 'error' && (
            <button className="toast-close" onClick={() => onDismiss(t.id)} aria-label="Close">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 12, height: 12 }}>
                <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          )}
        </div>
      ))}
    </div>
  )
}
