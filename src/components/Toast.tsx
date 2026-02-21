interface ToastProps {
  message: string
}

export function Toast({ message }: ToastProps) {
  return (
    <div
      className="fixed bottom-6 left-1/2 px-5 py-2.5 rounded-[10px] text-[13px] font-medium z-[999] pointer-events-none transition-all duration-300"
      style={{
        transform: `translateX(-50%) translateY(${message ? '0' : '20px'})`,
        opacity: message ? 1 : 0,
        background: 'var(--text)',
        color: 'var(--bg)',
        boxShadow: '0 8px 30px rgba(0,0,0,0.08), 0 0 1px rgba(0,0,0,0.1)',
      }}
    >
      {message}
    </div>
  )
}
