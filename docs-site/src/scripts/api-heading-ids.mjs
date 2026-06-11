import { visit } from 'unist-util-visit';

export default function apiHeadingIds() {
  return (tree) => {
    visit(tree, 'element', (node) => {
      if (!/^h[1-6]$/.test(node.tagName)) return;
      if (!Array.isArray(node.children) || node.children.length === 0) return;

      const markerIndex = node.children.findIndex((child) => getAnchor(child) !== undefined);
      if (markerIndex < 0) return;

      const anchor = getAnchor(node.children[markerIndex]);
      if (!anchor) return;

      node.properties ||= {};
      node.properties.id = anchor;
      node.children.splice(markerIndex, 1);
    });
  };
}

function getAnchor(node) {
  if (!node || typeof node !== 'object') return undefined;

  if (node.type === 'element' && node.tagName === 'span') {
    const properties = node.properties ?? {};
    return typeof properties['data-api-anchor'] === 'string' ? properties['data-api-anchor'] : undefined;
  }

  if (node.type === 'mdxJsxTextElement' && node.name === 'span') {
    const attribute = node.attributes?.find((candidate) => candidate.name === 'data-api-anchor');
    return typeof attribute?.value === 'string' ? attribute.value : undefined;
  }

  return undefined;
}
