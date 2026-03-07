import * as Dialog from '@radix-ui/react-dialog'
import { Copy, Link2, Loader2, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useAuth } from '../hooks/use-auth'
import { useConfig } from '../hooks/use-config'
import { useCreateShare } from '../hooks/use-share'
import { getAccessToken } from '../lib/supabase'
import { showToast } from '../lib/toast'
import { cn } from '../lib/utils'
// Use the SAME Message type as ConversationView (generated, not shared)
import type { Message } from '../types/generated'

// NO sessionTitle prop — derived internally from messages + projectName
interface ShareModalProps {
  sessionId: string
  messages?: Message[]
  projectName?: string
}

function getSessionTitle(messages?: Message[], projectName?: string): string {
  const firstUser = messages?.find((m) => m.role === 'user')
  if (firstUser?.content) {
    const truncated = firstUser.content.slice(0, 60).replace(/\s+\S*$/, '')
    return truncated || projectName || 'a Claude session'
  }
  return projectName || 'a Claude session'
}

// Dialog.Root wires onOpenChange to trigger share creation
export function ShareModal({ sessionId, messages, projectName }: ShareModalProps) {
  const { sharing } = useConfig()
  const [isOpen, setIsOpen] = useState(false)
  const [shareUrl, setShareUrl] = useState<string | null>(null)
  const [linkCopied, setLinkCopied] = useState(false)
  const [msgCopied, setMsgCopied] = useState(false)
  const { openSignIn } = useAuth()
  const createShare = useCreateShare()

  const sessionTitle = getSessionTitle(messages, projectName)

  // Plain async function (not useCallback) — avoids stale closure when
  // openSignIn(() => handleShare()) re-invokes after auth completes.
  const handleShare = async () => {
    try {
      const result = await createShare.mutateAsync(sessionId)
      setShareUrl(result.url)
    } catch (err: unknown) {
      if (err instanceof Error && err.message === 'AUTH_REQUIRED') {
        const token = await getAccessToken()
        if (token) {
          showToast('Share failed: server authentication error')
        } else {
          openSignIn(() => handleShare())
        }
      } else {
        const msg = err instanceof Error ? err.message : 'Unknown error'
        console.error('[share] failed:', msg)
        showToast(`Share failed: ${msg}`, 4000)
      }
    }
  }

  // Auto-trigger share creation when modal opens
  useEffect(() => {
    if (isOpen && !shareUrl && !createShare.isPending) {
      handleShare()
    }
  }, [isOpen]) // biome-ignore lint/correctness/useExhaustiveDependencies: intentional — only re-trigger on modal open

  const shareMessage = shareUrl
    ? `Check out my Claude session about "${sessionTitle}":\n${shareUrl}\n\nShared via claude-view`
    : ''

  const copyLink = async () => {
    if (!shareUrl) return
    await navigator.clipboard.writeText(shareUrl)
    setLinkCopied(true)
    setTimeout(() => setLinkCopied(false), 2000)
  }

  const copyMessage = async () => {
    await navigator.clipboard.writeText(shareMessage)
    setMsgCopied(true)
    setTimeout(() => setMsgCopied(false), 2000)
  }

  // Local mode — show disabled button with explanation
  if (!sharing) {
    return (
      <button
        type="button"
        disabled
        title="Sharing is not available in local mode"
        className="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-sm font-medium text-gray-400 dark:text-gray-500 bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md cursor-not-allowed opacity-60"
      >
        <Link2 className="w-4 h-4" /> Share
      </button>
    )
  }

  return (
    <Dialog.Root open={isOpen} onOpenChange={setIsOpen}>
      <Dialog.Trigger asChild>
        <button
          type="button"
          className="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 border border-gray-200 dark:border-gray-700 rounded-md transition-colors"
        >
          <Link2 className="w-4 h-4" /> Share
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 z-50" />
        <Dialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-900 rounded-lg p-6 w-full max-w-md z-50 shadow-xl">
          <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Share Conversation
          </Dialog.Title>
          <Dialog.Close className="absolute top-4 right-4 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
            <X className="w-4 h-4" />
          </Dialog.Close>

          {createShare.isPending ? (
            <div className="flex items-center gap-2 mt-4 text-sm text-gray-500">
              <Loader2 className="w-4 h-4 animate-spin" /> Creating share link...
            </div>
          ) : shareUrl ? (
            <div className="mt-4 space-y-4">
              {/* Copy Link */}
              <div className="flex gap-2">
                <input
                  readOnly
                  value={shareUrl}
                  className="flex-1 text-sm px-3 py-2 rounded-md border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-gray-100"
                />
                <button
                  type="button"
                  onClick={copyLink}
                  className={cn(
                    'px-3 py-2 text-sm rounded-md border transition-colors',
                    linkCopied
                      ? 'bg-green-50 dark:bg-green-900/30 text-green-700 dark:text-green-400 border-green-200 dark:border-green-800'
                      : 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 border-gray-200 dark:border-gray-700 hover:bg-gray-200 dark:hover:bg-gray-700',
                  )}
                >
                  {linkCopied ? 'Copied!' : 'Copy Link'}
                </button>
              </div>

              {/* Divider */}
              <div className="flex items-center gap-3 text-xs text-gray-400">
                <div className="flex-1 border-t border-gray-200 dark:border-gray-700" />
                or share with context
                <div className="flex-1 border-t border-gray-200 dark:border-gray-700" />
              </div>

              {/* Copy Message */}
              <textarea
                readOnly
                value={shareMessage}
                rows={4}
                className="w-full text-sm px-3 py-2 rounded-md border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-gray-100 resize-none"
              />
              <button
                type="button"
                onClick={copyMessage}
                className={cn(
                  'w-full px-3 py-2 text-sm rounded-md border transition-colors',
                  msgCopied
                    ? 'bg-green-50 dark:bg-green-900/30 text-green-700 dark:text-green-400 border-green-200 dark:border-green-800'
                    : 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 border-gray-200 dark:border-gray-700 hover:bg-gray-200 dark:hover:bg-gray-700',
                )}
              >
                <Copy className="w-4 h-4 inline mr-1.5" />
                {msgCopied ? 'Copied!' : 'Copy Message'}
              </button>
            </div>
          ) : null}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
