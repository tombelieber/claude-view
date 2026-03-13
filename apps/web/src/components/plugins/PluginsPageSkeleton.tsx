/** Skeleton placeholder shown while the plugins query loads on first render. */
export function PluginsPageSkeleton() {
  return (
    <div className="min-h-full bg-apple-bg animate-pulse">
      {/* Header */}
      <div className="px-7 pt-6 pb-0">
        <div className="h-8 w-28 bg-apple-sep2 rounded-lg" />
        <div className="h-4 w-56 bg-apple-sep2 rounded mt-1.5" />
      </div>

      {/* Toolbar */}
      <div className="px-7 pt-4 pb-2 flex gap-2">
        <div className="h-8 w-48 bg-apple-sep2 rounded-lg" />
        <div className="h-8 w-24 bg-apple-sep2 rounded-lg" />
        <div className="h-8 w-24 bg-apple-sep2 rounded-lg" />
      </div>

      {/* Cards */}
      <div className="px-7 py-5">
        <div className="h-5 w-36 bg-apple-sep2 rounded mb-3" />
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <div
              key={i}
              className="rounded-xl border border-apple-sep2 bg-white px-4 py-3.5 space-y-2"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className="h-4 w-20 bg-apple-sep2 rounded" />
                  <div className="h-3.5 w-10 bg-apple-sep2 rounded" />
                </div>
                <div className="h-5 w-8 bg-apple-sep2 rounded-full" />
              </div>
              <div className="h-3 w-44 bg-apple-sep2 rounded" />
              <div className="h-3 w-32 bg-apple-sep2 rounded" />
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
