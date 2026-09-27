import Link from 'next/link';

import { LiveDemo } from '@/components/live-demo';

export default function HomePage() {
  return (
    <main className="hero-grid min-h-screen">
      <div className="mx-auto max-w-6xl px-6 py-16 sm:py-24">
        <header className="mb-16 flex items-center justify-between text-sm">
          <span className="flex items-center gap-2 font-semibold">
            <span className="text-lg text-teal-600">◉</span>
            MolGFX
          </span>
          <a href="https://miguelcsx.github.io/molframe/" className="text-fd-muted-foreground hover:text-fd-foreground">
            MolFrame ↗
          </a>
        </header>
        <section className="max-w-3xl">
          <p className="mb-5 font-mono text-sm text-teal-700 dark:text-teal-300">
            MOLECULAR VISUALIZATION · PYTHON + RUST + WEBGPU
          </p>
          <h1 className="text-5xl font-semibold tracking-tight sm:text-6xl">
            Render molecular structure with scientific intent.
          </h1>
          <p className="mt-7 max-w-2xl text-lg leading-8 text-fd-muted-foreground">
            MolGFX turns MolFrame structures into interactive WebGPU scenes and deterministic images.
          </p>
          <div className="mt-9 flex flex-wrap gap-3">
            <Link href="/docs" className="rounded-lg bg-fd-primary px-5 py-3 font-medium text-fd-primary-foreground">
              Read the docs
            </Link>
            <a href="https://github.com/miguelcsx/molgfx" className="rounded-lg border border-fd-border px-5 py-3 font-medium">
              View on GitHub
            </a>
          </div>
        </section>
        <section className="mt-20" aria-labelledby="workbench-title">
          <div className="mb-5 max-w-3xl">
            <p className="font-mono text-sm text-teal-700 dark:text-teal-300">BROWSER WORKBENCH</p>
            <h2 id="workbench-title" className="mt-2 text-3xl font-semibold tracking-tight">Explore an example, or replace it with your structure.</h2>
          </div>
          <LiveDemo />
        </section>
      </div>
    </main>
  );
}