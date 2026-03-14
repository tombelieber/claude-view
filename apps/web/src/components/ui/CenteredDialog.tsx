import * as AlertDialog from '@radix-ui/react-alert-dialog'
import * as Dialog from '@radix-ui/react-dialog'
import { type ComponentPropsWithoutRef, forwardRef } from 'react'
import { cn } from '../../lib/utils'

/**
 * Pre-centered Dialog/AlertDialog primitives.
 * Enforces the mandatory centering pattern via inline transform
 * (immune to Tailwind v4 translate composability issues).
 *
 * Only bakes in positioning (fixed + z-index + centering).
 * All visual styles (bg, border, rounded, padding, shadow) come from className.
 *
 * Usage:
 *   <Dialog.Root>
 *     <Dialog.Portal>
 *       <DialogOverlay />
 *       <DialogContent className="max-w-md rounded-lg bg-white shadow-xl p-6">
 *         ...
 *       </DialogContent>
 *     </Dialog.Portal>
 *   </Dialog.Root>
 */

// ── Dialog ──────────────────────────────────────

export const DialogOverlay = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<typeof Dialog.Overlay>
>(({ className, ...props }, ref) => (
  <Dialog.Overlay
    ref={ref}
    className={cn('fixed inset-0 z-50 bg-black/40', className)}
    {...props}
  />
))
DialogOverlay.displayName = 'DialogOverlay'

export const DialogContent = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<typeof Dialog.Content>
>(({ className, style, ...props }, ref) => (
  <Dialog.Content
    ref={ref}
    className={cn('fixed z-[51] top-1/2 left-1/2 w-full focus:outline-none', className)}
    style={{ ...style, transform: 'translate(-50%, -50%)' }}
    {...props}
  />
))
DialogContent.displayName = 'DialogContent'

// ── AlertDialog ─────────────────────────────────

export const AlertDialogOverlay = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<typeof AlertDialog.Overlay>
>(({ className, ...props }, ref) => (
  <AlertDialog.Overlay
    ref={ref}
    className={cn('fixed inset-0 z-50 bg-black/40', className)}
    {...props}
  />
))
AlertDialogOverlay.displayName = 'AlertDialogOverlay'

export const AlertDialogContent = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<typeof AlertDialog.Content>
>(({ className, style, ...props }, ref) => (
  <AlertDialog.Content
    ref={ref}
    className={cn('fixed z-[51] top-1/2 left-1/2 w-full focus:outline-none', className)}
    style={{ ...style, transform: 'translate(-50%, -50%)' }}
    {...props}
  />
))
AlertDialogContent.displayName = 'AlertDialogContent'
