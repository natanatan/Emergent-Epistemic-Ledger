import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://eel.emergentexistence.com',
  trailingSlash: 'ignore',
  build: { format: 'directory' },
});
