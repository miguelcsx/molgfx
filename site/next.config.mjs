import { createMDX } from 'fumadocs-mdx/next';

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
const config = {
  output: 'export',
  basePath: '/molgfx',
  assetPrefix: '/molgfx/',
  trailingSlash: true,
  reactStrictMode: true,
};

export default withMDX(config);
