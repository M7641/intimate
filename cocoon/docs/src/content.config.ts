import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';

// One `docs` collection backed by the Markdown tree under src/content/docs.
// The frontmatter shape mirrors the subset of Starlight's we actually use, so
// existing content needs no rewrite.
const docs = defineCollection({
  loader: glob({ pattern: '**/*.md', base: './src/content/docs' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    sidebar: z
      .object({
        label: z.string().optional(),
        order: z.number().default(0),
      })
      .default({ order: 0 }),
  }),
});

export const collections = { docs };
