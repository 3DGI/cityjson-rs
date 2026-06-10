import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://cityjson.3dgi.nl',
  integrations: [
    starlight({
      title: 'cityjson-rs',
      description: 'Rust, Python, C++, WASM, Arrow, and Parquet documentation for cityjson-rs.',
      logo: {
        src: './public/assets/3dgi-logo.png',
        alt: '3DGI',
      },
      customCss: ['./src/styles/custom.css'],
      components: {
        Header: './src/components/Header.astro',
        MobileMenuFooter: './src/components/MobileMenuFooter.astro',
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/3DGI/cityjson-rs',
        },
      ],
      sidebar: [
        {
          label: 'Overview',
          items: [
            { label: 'Start here', slug: 'index' },
            { label: 'API reference', slug: 'reference' },
          ],
        },
        {
          label: 'cityjson-lib',
          items: [{ label: 'Usage across languages', slug: 'lib/cityjson-lib' }],
        },
        {
          label: 'cityjson-index',
          items: [{ label: 'Usage across languages', slug: 'index/cityjson-index' }],
        },
        {
          label: 'API reference',
          items: [
            {
              label: 'cityjson-lib',
              items: [
                { label: 'Rust', slug: 'reference/cityjson-lib/rust' },
                { label: 'Python', slug: 'reference/cityjson-lib/python' },
                { label: 'C++', slug: 'reference/cityjson-lib/cpp' },
                { label: 'WASM', slug: 'reference/cityjson-lib/wasm' },
                { label: 'C FFI', slug: 'reference/cityjson-lib/c' },
              ],
            },
            {
              label: 'cityjson-index',
              items: [
                { label: 'Rust', slug: 'reference/cityjson-index/rust' },
                { label: 'Python', slug: 'reference/cityjson-index/python' },
                { label: 'C FFI', slug: 'reference/cityjson-index/c' },
              ],
            },
          ],
        },
        {
          label: 'cityjson-arrow specs',
          items: [
            { label: 'Overview', slug: 'arrow' },
            { label: 'Arrow IPC layout', slug: 'arrow/specs/cityjson-arrow-ipc-spec' },
            { label: 'Package layout', slug: 'arrow/specs/package-spec' },
            { label: 'Package schema', slug: 'arrow/specs/package-schema' },
          ],
        },
      ],
    }),
    mdx(),
  ],
});
