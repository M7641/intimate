// @ts-check
import { defineConfig } from 'astro/config';
import pagefind from 'astro-pagefind';
import remarkDirective from 'remark-directive';
import { remarkAsides } from './src/plugins/remark-asides.mjs';

// Plain Astro — no docs framework. The Starlight *look* is reproduced by our own
// layouts/components/CSS; the only Starlight content feature we keep is asides
// (`:::note`), via a small remark plugin so the content stays pure Markdown.
// Pagefind (the same static indexer Starlight used) restores full-text search.
export default defineConfig({
  integrations: [pagefind()],
  markdown: {
    remarkPlugins: [remarkDirective, remarkAsides],
    // Dual Shiki theme, switched by the `data-theme` attribute (see global.css).
    shikiConfig: {
      themes: { light: 'github-light', dark: 'github-dark' },
      defaultColor: false,
    },
  },
});
