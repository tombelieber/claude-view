/**
 * Shows a temporary toast notification at the bottom-right of the screen.
 * Uses only inline styles — no style injection, no DOM leaks.
 */
export function showToast(message: string, duration = 2000): void {
  const toast = document.createElement('div')
  toast.textContent = message
  toast.style.cssText = [
    'position: fixed',
    'bottom: 20px',
    'right: 20px',
    'background-color: #059669',
    'color: white',
    'padding: 12px 16px',
    'border-radius: 6px',
    'font-size: 14px',
    'font-family: -apple-system, BlinkMacSystemFont, sans-serif',
    'font-weight: 500',
    'box-shadow: 0 4px 6px rgba(0,0,0,0.1)',
    'z-index: 9999',
    'opacity: 0',
    'transform: translateY(10px)',
    'transition: opacity 0.2s ease-out, transform 0.2s ease-out',
  ].join(';')

  document.body.appendChild(toast)

  requestAnimationFrame(() => {
    toast.style.opacity = '1'
    toast.style.transform = 'translateY(0)'
  })

  setTimeout(() => {
    toast.style.opacity = '0'
    toast.style.transform = 'translateY(10px)'
    const onEnd = () => {
      toast.removeEventListener('transitionend', onEnd)
      toast.remove()
    }
    toast.addEventListener('transitionend', onEnd)
    setTimeout(() => toast.remove(), 500)
  }, duration)
}
