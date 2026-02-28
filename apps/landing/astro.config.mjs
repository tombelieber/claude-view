// NOTE: site URL and GitHub URL are also defined in src/data/site.ts (single source of truth).
// astro.config.mjs cannot import .ts directly, so values are duplicated here.
import sitemap from '@astrojs/sitemap'
import starlight from '@astrojs/starlight'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'astro/config'

export default defineConfig({
  site: 'https://claude-view.dev',
  trailingSlash: 'always',
  prefetch: {
    defaultStrategy: 'hover',
    prefetchAll: true,
  },
  experimental: {
    clientPrerender: true,
  },
  integrations: [
    starlight({
      title: 'claude-view',
      description: 'Mission Control for AI coding agents',
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/tombelieber/claude-view' },
      ],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Introduction', slug: 'docs' },
            { label: 'Installation', slug: 'docs/installation' },
          ],
        },
        {
          label: 'Features',
          autogenerate: { directory: 'docs/features' },
        },
        {
          label: 'Guides',
          autogenerate: { directory: 'docs/guides' },
        },
        {
          label: 'Reference',
          autogenerate: { directory: 'docs/reference' },
        },
      ],
      customCss: ['./src/styles/starlight.css'],
      head: [
        {
          tag: 'script',
          attrs: { type: 'application/ld+json' },
          content: JSON.stringify({
            '@context': 'https://schema.org',
            '@type': 'TechArticle',
            isPartOf: {
              '@type': 'SoftwareApplication',
              name: 'claude-view',
              applicationCategory: 'DeveloperApplication',
            },
          }),
        },
        {
          tag: 'script',
          attrs: { type: 'application/ld+json' },
          content: JSON.stringify({
            '@context': 'https://schema.org',
            '@type': 'BreadcrumbList',
            itemListElement: [
              { '@type': 'ListItem', position: 1, name: 'Home', item: 'https://claude-view.dev/' },
              {
                '@type': 'ListItem',
                position: 2,
                name: 'Documentation',
                item: 'https://claude-view.dev/docs/',
              },
              { '@type': 'ListItem', position: 3, name: 'Current Page' },
            ],
          }),
        },
      ],
    }),
    sitemap(),
  ],
  vite: {
    plugins: [tailwindcss()],
  },
})
