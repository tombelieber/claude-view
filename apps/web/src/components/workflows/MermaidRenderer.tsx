import mermaid from 'mermaid'
import { useEffect, useId, useRef } from 'react'

mermaid.initialize({ startOnLoad: false, theme: 'dark', securityLevel: 'loose' })

interface MermaidRendererProps {
  chart: string
}

export function MermaidRenderer({ chart }: MermaidRendererProps) {
  const uid = useId().replace(/:/g, 'm')
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!ref.current || !chart.trim()) return
    const render = async () => {
      try {
        const { svg } = await mermaid.render(uid, chart)
        if (!ref.current) return
        const parser = new DOMParser()
        const doc = parser.parseFromString(svg, 'image/svg+xml')
        const svgEl = doc.querySelector('svg')
        if (svgEl && ref.current) {
          ref.current.replaceChildren(svgEl)
        }
      } catch {
        if (ref.current) ref.current.replaceChildren()
      }
    }
    void render()
  }, [chart, uid])

  return <div ref={ref} className="w-full overflow-auto min-h-[200px]" />
}
