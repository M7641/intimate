// The hero guides in docs/guides/, one Markdown file per hero. Each is its
// own chunk, fetched when its tab is first opened.
const GUIDES = import.meta.glob<string>(["../../../docs/guides/*.md", "!**/README.md"], {
  query: "?raw",
  import: "default",
});

const path = (hero: string) => `../../../docs/guides/${hero}.md`;

export const hasGuide = (hero: string) => path(hero) in GUIDES;

export const loadGuide = (hero: string): Promise<string> => GUIDES[path(hero)]();
