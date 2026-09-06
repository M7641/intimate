import { visit } from 'unist-util-visit';

// The aside variants we support, with their default titles.
const ASIDE_LABELS = {
  note: 'Note',
  tip: 'Tip',
  caution: 'Caution',
  danger: 'Danger',
};

/**
 * Turn `:::note` / `:::caution[Custom title]` container directives into styled
 * `<aside>` elements — a hand-rolled clone of Starlight's asides, so the content
 * stays plain Markdown. Pure remark: no runtime component, no MDX.
 *
 * A unified plugin: a function returning the tree transformer.
 */
export function remarkAsides() {
  return (tree) => {
    visit(tree, (node) => {
      if (node.type !== 'containerDirective') return;
      const variant = node.name;
      if (!Object.hasOwn(ASIDE_LABELS, variant)) return;

      // An optional custom title is parsed by remark-directive as a leading
      // paragraph flagged `directiveLabel` — e.g. `:::note[My title]`.
      let title = ASIDE_LABELS[variant];
      const first = node.children[0];
      if (first?.type === 'paragraph' && first.data?.directiveLabel) {
        const text = first.children.find((c) => c.type === 'text');
        if (text) title = text.value;
        node.children.shift();
      }

      node.children.unshift({
        type: 'paragraph',
        data: { hName: 'p', hProperties: { class: 'aside__title' } },
        children: [{ type: 'text', value: title }],
      });

      const data = node.data || (node.data = {});
      data.hName = 'aside';
      data.hProperties = { class: `aside aside--${variant}` };
    });
  };
}
