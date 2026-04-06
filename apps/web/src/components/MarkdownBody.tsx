import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'

/**
 * Simple markdown renderer for memory files and similar content.
 * Uses react-markdown with GFM support.
 */
export function MarkdownBody({ content }: { content: string }) {
  return <Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown>
}
