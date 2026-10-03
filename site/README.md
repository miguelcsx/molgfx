# site

This is a Next.js application generated with
[Create Fumadocs](https://github.com/fuma-nama/fumadocs).

It is a Next.js app with [Static Export](https://nextjs.org/docs/app/guides/static-exports) configured.

Run development server:

```bash
npm run dev
# or
pnpm dev
# or
yarn dev
```

Open http://localhost:3000 with your browser to see the result.

## Public browser package

The live demo imports `Viewer` from `molgfx` and `molgfx/viewer.css`. Next.js
bundles the same canonical npm distribution used by notebooks and emits its
byte-identical wasm under `/molgfx/_next/`; there is no private `/runtime` copy.
The site deliberately uses `file:../web` until the first npm publication. This
keeps a clean checkout buildable without pretending an unpublished package is
available. Cargo, Python and the web package must share one release version.

From the repository root, build the canonical frontend before the site:

```bash
nix develop -c npm ci --prefix web
nix develop -c npm run build --prefix web
nix develop -c npm ci --prefix site
nix develop -c npm run build --prefix site
nix develop -c npx --prefix web playwright install chromium
nix develop -c npm run test:demo --prefix site
```

The docs use Next's webpack bundler, which supports the local package link and
the package's static wasm URL. The production smoke checks all public JS
entrypoints in Node, rejects private package paths, compares emitted wasm
hashes, renders the real RCSB haemoglobin structure through WebGPU, and
navigates away to exercise teardown. It saves `site/test-results/demo.png`.
The smoke requires network access to RCSB and a WebGPU-capable Chromium; CI
uses the shared browser launch options and installs Linux browser dependencies.

After a maintainer publishes the matching package version, replace the local
dependency and regenerate the lockfile with an exact registry version:

```bash
version=$(node -p 'require("./web/package.json").version')
npm install --save-exact --prefix site "molgfx@$version"
npm ci --prefix site
npm run build --prefix site
npm run test:demo --prefix site
```

### npm release authorization

`release-npm.yml` detects unpublished workspace versions on main, checks
package artifacts and packed production consumers, then publishes the verified
tarball through OIDC with provenance. Existing versions are never overwritten;
registry errors fail closed. Manual runs default to `dry-run: true` and still
verify an already-published version. Non-main refs cannot publish.

The registry returned 404 for `molgfx` during setup. A maintainer must authorize
the first publication separately, then configure the package's trusted
publisher for owner `miguelcsx`, repository `molgfx`, workflow
`release-npm.yml`, and environment `npm`. Protect that GitHub environment with
the required release approvers. The workflow has no token fallback and fails
with an explicit prerequisite error before publishing an unbootstrapped name.
Trusted publishing requires a modern npm CLI; the publishing job pins an
OIDC-capable npm release on Node 22. See the
[npm trusted publisher documentation](https://docs.npmjs.com/trusted-publishers).

## Explore

In the project, you can see:

- `lib/source.ts`: Code for content source adapter, [`loader()`](https://fumadocs.dev/docs/headless/source-api) provides the interface to access your content.
- `lib/layout.shared.tsx`: Shared options for layouts, optional but preferred to keep.

| Route                     | Description                                            |
| ------------------------- | ------------------------------------------------------ |
| `app/(home)`              | The route group for your landing page and other pages. |
| `app/docs`                | The documentation layout and pages.                    |
| `app/api/search/route.ts` | The Route Handler for search.                          |

### Fumadocs MDX

Collections are defined with the [Macro API](https://fumadocs.dev/docs/mdx/macro) in `lib/source.ts`.

Read the [Introduction](https://fumadocs.dev/docs/mdx) for further details.

## Learn More

To learn more about Next.js and Fumadocs, take a look at the following
resources:

- [Next.js Documentation](https://nextjs.org/docs) - learn about Next.js
  features and API.
- [Learn Next.js](https://nextjs.org/learn) - an interactive Next.js tutorial.
- [Fumadocs](https://fumadocs.dev) - learn about Fumadocs
