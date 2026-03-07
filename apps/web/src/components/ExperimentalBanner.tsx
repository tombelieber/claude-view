import { Banner } from './ui/Banner'

const DISMISSED_KEY = 'experimental-insights-banner-dismissed'

interface ExperimentalBannerProps {
  storageKey?: string
}

export function ExperimentalBanner({ storageKey = DISMISSED_KEY }: ExperimentalBannerProps) {
  return (
    <Banner variant="experimental" dismissKey={storageKey} className="mb-4">
      <p className="font-medium">Experimental Feature</p>
      <p className="text-xs opacity-80 mt-0.5">
        AI-powered insights and session classification are early-stage. Results may be inaccurate,
        incomplete, or change as the feature matures. Use as a rough guide, not ground truth.
      </p>
    </Banner>
  )
}
