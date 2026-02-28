import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import sitemap from '@astrojs/sitemap';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  site: 'https://claude-view.dev',
  trailingSlash: 'always',
  integrations: [
    starlight({
      title: 'claude-view',
      description: 'Mission Control for AI coding agents',
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/anthropics/claude-view' },
      ],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Introduction', slug: '' },
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
            "@context": "https://schema.org",
            "@type": "TechArticle",
            "isPartOf": {
              "@type": "SoftwareApplication",
              "name": "claude-view",
              "applicationCategory": "DeveloperApplication"
            }
          }),
        },
      ],
    }),
    sitemap(),
  ],
  vite: {
    plugins: [tailwindcss()],
  },
});
