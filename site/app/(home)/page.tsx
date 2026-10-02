import Link from 'next/link';

export default function HomePage() {
  return (
    <main className="hero-grid min-h-screen">
      <div className="mx-auto max-w-6xl px-6 py-8 sm:py-12">
        <header className="mb-14 flex items-center justify-between text-sm">
          <span className="flex items-center gap-2 font-semibold">
            <span className="text-lg text-teal-600">◉</span>
            MolGFX
          </span>
          <a href="https://miguelcsx.github.io/molframe/" className="text-fd-muted-foreground hover:text-fd-foreground">
            MolFrame ↗
          </a>
        </header>
        <section className="grid items-center gap-12 lg:grid-cols-[minmax(0,0.82fr)_minmax(0,1.18fr)] lg:gap-16">
          <div className="max-w-xl">
            <p className="mb-5 font-mono text-sm text-teal-700 dark:text-teal-300">
              MOLECULAR VISUALIZATION · PYTHON + RUST + WEBGPU
            </p>
            <h1 className="text-5xl font-semibold tracking-tight sm:text-6xl">
              Molecular structure, rendered with intent.
            </h1>
            <p className="mt-7 max-w-lg text-lg leading-8 text-fd-muted-foreground">
              MolGFX turns MolFrame structures into GPU-rendered scenes and deterministic images, for the application you build around them.
            </p>
            <div className="mt-9 flex flex-wrap gap-3">
              <Link href="/docs" className="rounded-lg bg-fd-primary px-5 py-3 font-medium text-fd-primary-foreground">
                Read the docs
              </Link>
              <a href="https://github.com/miguelcsx/molgfx" className="rounded-lg border border-fd-border px-5 py-3 font-medium">
                View on GitHub
              </a>
            </div>
            <dl className="mt-12 grid grid-cols-3 gap-4 border-t border-fd-border pt-5 text-sm">
              <div>
                <dt className="font-mono text-xs text-fd-muted-foreground">BACKEND</dt>
                <dd className="mt-1 font-medium">WebGPU</dd>
              </div>
              <div>
                <dt className="font-mono text-xs text-fd-muted-foreground">INPUT</dt>
                <dd className="mt-1 font-medium">PDB / mmCIF</dd>
              </div>
              <div>
                <dt className="font-mono text-xs text-fd-muted-foreground">OUTPUT</dt>
                <dd className="mt-1 font-medium">Images / frames</dd>
              </div>
            </dl>
          </div>
          <figure className="figure-card">
            <img src="/molgfx/images/haemoglobin-pocket.png" alt="Haemoglobin rendered by MolGFX with heme groups and an orange local protein pocket" />
            <figcaption><strong>Haemoglobin, PDB 4HHB</strong>Cartoon context, space-filling hemes and the pocket around them, rendered by MolGFX.</figcaption>
          </figure>
        </section>
        <section className="mt-16 grid gap-4 border-t border-fd-border pt-8 sm:grid-cols-3">
          <Link href="/docs/getting-started/first-scene" className="home-path-card">
            <span className="font-mono text-xs text-teal-700 dark:text-teal-300">01 · START</span>
            <strong>Load a structure</strong>
            <span>Build the first scene from coordinates and topology.</span>
          </Link>
          <Link href="/docs/representations/overview" className="home-path-card">
            <span className="font-mono text-xs text-teal-700 dark:text-teal-300">02 · DESIGN</span>
            <strong>Choose a representation</strong>
            <span>Atoms, bonds, ribbons, and material controls.</span>
          </Link>
          <Link href="/docs/rendering/images" className="home-path-card">
            <span className="font-mono text-xs text-teal-700 dark:text-teal-300">03 · EXPORT</span>
            <strong>Produce a frame</strong>
            <span>Render deterministic images for your workflow.</span>
          </Link>
        </section>
      </div>
    </main>
  );
}