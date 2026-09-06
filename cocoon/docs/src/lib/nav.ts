import { getCollection } from 'astro:content';

export interface NavItem {
  label: string;
  href: string;
}

export interface NavGroup {
  label: string;
  items: NavItem[];
}

// The sidebar groups, in display order. Each maps to a content sub-directory;
// adding a Markdown file under one of these shows up automatically.
const GROUPS: { label: string; dir: string }[] = [
  { label: 'Applications', dir: 'apps' },
  { label: 'Decisions', dir: 'decisions' },
];

/** Build the grouped sidebar from the docs collection, sorted by frontmatter. */
export async function buildNav(): Promise<NavGroup[]> {
  const entries = await getCollection('docs');

  return GROUPS.map(({ label, dir }) => {
    const items = entries
      .filter((entry) => entry.id.startsWith(`${dir}/`))
      .sort(
        (a, b) =>
          a.data.sidebar.order - b.data.sidebar.order ||
          a.data.title.localeCompare(b.data.title),
      )
      .map((entry) => ({
        label: entry.data.sidebar.label ?? entry.data.title,
        href: `/${entry.id}/`,
      }));

    return { label, items };
  });
}
