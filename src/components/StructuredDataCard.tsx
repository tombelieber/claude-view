import { useMemo } from 'react'
import DOMPurify from 'dompurify'

interface StructuredDataCardProps {
  xml: string | null | undefined
  type?: 'unknown' | 'observation' | 'tool_call' | 'command'
}

/**
 * Safely renders untrusted XML/HTML content as plain text.
 *
 * Security: Strips ALL tags and attributes via DOMPurify, then renders
 * the text content through React (auto-escaped). No dangerouslySetInnerHTML.
 */
export function StructuredDataCard({
  xml,
}: StructuredDataCardProps) {
  // Strip all HTML/XML tags, keep only text content
  const plainText = useMemo(
    () => {
      if (!xml || !xml.trim()) {
        return ''
      }
      return DOMPurify.sanitize(xml, {
        ALLOWED_TAGS: [],
        ALLOWED_ATTR: [],
        KEEP_CONTENT: true,
      })
    },
    [xml]
  )

  // Handle empty/null/undefined content
  if (!plainText.trim()) {
    return <div className="text-gray-500">No data</div>
  }

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-3 my-2">
      <div className="font-mono text-sm text-gray-700">
        <pre
          className="whitespace-pre-wrap break-words text-xs bg-gray-50 p-2 rounded border border-gray-100"
          role="document"
          aria-label="Structured data content"
        >
          {plainText}
        </pre>
      </div>
    </div>
  )
}
