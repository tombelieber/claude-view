import { defineCollection } from 'astro:content'
import { docsLoader } from '@astrojs/starlight/loaders'
import { docsSchema } from '@astrojs/starlight/schema'
import { glob } from 'astro/loaders'
import { z } from 'astro/zod'

export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    schema: docsSchema(),
  }),
  blog: defineCollection({
    loader: glob({ pattern: '**/*.mdx', base: './src/content/blog' }),
    schema: z.object({
      title: z.string(),
      description: z.string(),
      date: z.coerce.date(),
      author: z.string().default('claude-view team'),
    }),
  }),
  changelog: defineCollection({
    loader: glob({ pattern: '**/*.md', base: './src/content/changelog' }),
    schema: z.object({
      title: z.string(),
      date: z.coerce.date(),
      version: z.string(),
    }),
  }),
}
