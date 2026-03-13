interface AppleToggleProps {
  checked: boolean
  onChange: () => void
  disabled?: boolean
  size?: 'sm' | 'md'
}

export function AppleToggle({
  checked,
  onChange,
  disabled = false,
  size = 'md',
}: AppleToggleProps) {
  const isSm = size === 'sm'
  const trackW = isSm ? 'w-[30px]' : 'w-[40px]'
  const trackH = isSm ? 'h-[18px]' : 'h-[24px]'
  const thumbSize = isSm ? 'w-[14px] h-[14px]' : 'w-[20px] h-[20px]'
  const thumbTranslate = checked ? `translateX(${isSm ? 14 : 18}px)` : 'translateX(2px)'

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={onChange}
      disabled={disabled}
      className={`${trackW} ${trackH} relative inline-flex items-center rounded-full transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-apple-blue disabled:opacity-50 disabled:cursor-not-allowed ${checked ? 'bg-apple-blue' : 'bg-apple-sep'}`}
    >
      <span
        className={`${thumbSize} block rounded-full bg-white shadow-sm transition-transform duration-200`}
        style={{ transform: thumbTranslate }}
      />
    </button>
  )
}
