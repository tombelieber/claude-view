import { Paperclip, X } from 'lucide-react'
import { useRef } from 'react'
import { cn } from '../../lib/utils'

interface AttachButtonProps {
  onAttach: (files: File[]) => void
  disabled?: boolean
}

/** Accepted file types for attachment. */
const ACCEPT =
  '.txt,.md,.json,.ts,.tsx,.js,.jsx,.py,.rs,.go,.css,.html,.yaml,.yml,.toml,.csv,.log,.sh,.sql,.xml,.env,.cfg,.ini,.diff,.patch,.png,.jpg,.jpeg,.gif,.webp,.pdf'

/**
 * Paperclip button that opens a hidden file input for attachments.
 */
export function AttachButton({ onAttach, disabled }: AttachButtonProps) {
  const inputRef = useRef<HTMLInputElement>(null)

  function handleClick() {
    inputRef.current?.click()
  }

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const fileList = e.target.files
    if (fileList && fileList.length > 0) {
      onAttach(Array.from(fileList))
    }
    // Reset input so re-selecting the same file triggers change
    if (inputRef.current) {
      inputRef.current.value = ''
    }
  }

  return (
    <>
      <button
        type="button"
        onClick={handleClick}
        disabled={disabled}
        className={cn(
          'p-1.5 rounded-md transition-colors duration-150',
          'text-gray-400 dark:text-gray-500',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
          disabled
            ? 'opacity-50 cursor-not-allowed'
            : 'hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 cursor-pointer',
        )}
        aria-label="Attach files"
      >
        <Paperclip className="w-4 h-4" aria-hidden="true" />
      </button>
      <input
        ref={inputRef}
        type="file"
        multiple
        accept={ACCEPT}
        onChange={handleChange}
        className="hidden"
        aria-hidden="true"
        tabIndex={-1}
      />
    </>
  )
}

interface AttachmentChipsProps {
  attachments: File[]
  onRemove: (index: number) => void
}

/**
 * Renders a row of file attachment chips with remove buttons.
 * Returns null when the attachments array is empty.
 */
export function AttachmentChips({ attachments, onRemove }: AttachmentChipsProps) {
  if (attachments.length === 0) return null

  return (
    <div className="flex flex-wrap gap-1.5 px-3 pb-2">
      {attachments.map((file, idx) => (
        <span
          key={`${file.name}-${idx}`}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 border border-gray-200 dark:border-gray-700"
        >
          <Paperclip className="w-3 h-3 text-gray-400" aria-hidden="true" />
          <span className="max-w-[120px] truncate">{file.name}</span>
          <button
            type="button"
            onClick={() => onRemove(idx)}
            className="p-0.5 rounded-full hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors cursor-pointer"
            aria-label={`Remove ${file.name}`}
          >
            <X className="w-3 h-3" aria-hidden="true" />
          </button>
        </span>
      ))}
    </div>
  )
}
