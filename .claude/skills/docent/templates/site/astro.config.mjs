import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import config from '../docent.config.json' with { type: 'json' };

const { owner, repo } = config.project;
const customDomain = config.deploy?.customDomain ?? null;

const site = customDomain
  ? `https://${customDomain}`
  : `https://${owner}.github.io`;

const base = customDomain ? '/' : `/${repo}`;

export default defineConfig({
  site,
  base,
  trailingSlash: 'always',
  integrations: [mdx()],
  vite: {
    server: {
      fs: {
        allow: ['..'],
      },
    },
  },
});
