# Website

The public site at <https://eel.emergentexistence.com>, built with Astro.

```sh
cd site
npm install
npm run dev      # http://localhost:4321
npm run build    # static files in site/dist
```

The white paper lives in `src/content/white-paper.md`. Every push to `main`
that touches `site/` builds the site and uploads it over SFTP
(`.github/workflows/site.yml`). The upload never deletes files on the server.
