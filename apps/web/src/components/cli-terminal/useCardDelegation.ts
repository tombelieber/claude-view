import { useCallback, useState } from 'react'
import {
  dispatchKeys,
  translateSelectOption,
  translateMultiSelect,
  translateFreeText,
  translatePlanApproval,
} from '../../lib/tmux-keys'

export type CardDelegationState = 'idle' | 'sending' | 'sent' | 'resolved'

interface UseCardDelegationOptions {
  sendKeys: (data: string) => void
  isConnected: boolean
}

export function useCardDelegation({ sendKeys, isConnected }: UseCardDelegationOptions) {
  const [state, setState] = useState<CardDelegationState>('idle')

  const delegateSelectOption = useCallback(
    async (index: number) => {
      if (!isConnected || state === 'sending') return
      setState('sending')
      const keys = translateSelectOption(index)
      await dispatchKeys(sendKeys, keys)
      setState('sent')
      setTimeout(() => setState('resolved'), 2000)
    },
    [sendKeys, isConnected, state],
  )

  const delegateMultiSelect = useCallback(
    async (indices: number[]) => {
      if (!isConnected || state === 'sending') return
      setState('sending')
      const keys = translateMultiSelect(indices)
      await dispatchKeys(sendKeys, keys)
      setState('sent')
      setTimeout(() => setState('resolved'), 2000)
    },
    [sendKeys, isConnected, state],
  )

  const delegateFreeText = useCallback(
    async (text: string) => {
      if (!isConnected || state === 'sending') return
      setState('sending')
      const keys = translateFreeText(text)
      await dispatchKeys(sendKeys, keys)
      setState('sent')
      setTimeout(() => setState('resolved'), 2000)
    },
    [sendKeys, isConnected, state],
  )

  const delegatePlanApproval = useCallback(
    async (approved: boolean) => {
      if (!isConnected || state === 'sending') return
      setState('sending')
      const keys = translatePlanApproval(approved)
      await dispatchKeys(sendKeys, keys)
      setState('sent')
      setTimeout(() => setState('resolved'), 2000)
    },
    [sendKeys, isConnected, state],
  )

  const reset = useCallback(() => setState('idle'), [])

  return {
    state,
    delegateSelectOption,
    delegateMultiSelect,
    delegateFreeText,
    delegatePlanApproval,
    reset,
  }
}
