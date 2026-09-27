import Link from 'next/link';

export default function HomePage() {
  return <main className="molgfx-hero flex flex-1 items-center justify-center px-6 py-20 text-center">
    <div className="max-w-3xl">
      <p className="mb-5 text-sm font-semibold tracking-[0.18em] text-fd-muted-foreground">MOLECULAR VISUALIZATION</p>
      <h1 className="mb-6 text-5xl font-semibold tracking-tight sm:text-6xl">See structure. Explain biology.</h1>
      <p className="mx-auto mb-9 max-w-2xl text-lg leading-8 text-fd-muted-foreground">MolGFX turns MolFrame structures into interactive WebGPU scenes, notebook views, and deterministic images—through a compact semantic API.</p>
      <div className="flex flex-wrap justify-center gap-3"><Link href="/docs" className="rounded-lg bg-fd-primary px-5 py-3 font-medium text-fd-primary-foreground">Read the docs</Link><a href="https://github.com/miguelcsx/molgfx" className="rounded-lg border border-fd-border px-5 py-3 font-medium">View on GitHub</a></div>
    </div>
  </main>;
}
