import { LiveDemo } from '@/components/live-demo';
import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { baseOptions } from '@/lib/layout.shared';
import { source } from '@/lib/source';

export const metadata = {
  title: 'Local-file Workbench demo',
  description: 'Render a local molecular structure in your browser with MolGFX.',
};

export default function DemoPage() {
  return (
    <DocsLayout tree={source.getPageTree()} {...baseOptions()}>
      <main className="mx-auto w-full max-w-5xl px-4 py-10 md:px-8">
        <p className="mb-3 text-sm font-semibold tracking-[0.16em] text-fd-muted-foreground">BROWSER WORKBENCH</p>
        <h1 className="mb-4 text-4xl font-semibold tracking-tight">Render a local structure</h1>
        <p className="mb-8 max-w-3xl text-lg leading-8 text-fd-muted-foreground">
          Open a local mmCIF, BinaryCIF, or PDB file. MolFrame parses it and MolGFX renders it in this browser; no structure bytes are uploaded.
        </p>
        <LiveDemo />
        <div className="mt-8 grid gap-4 text-sm text-fd-muted-foreground md:grid-cols-3">
          <p><strong className="text-fd-foreground">WebGPU required.</strong> Unsupported browsers show an actionable message before parsing.</p>
          <p><strong className="text-fd-foreground">Try commands.</strong> The console starts with a chain-coloured cartoon. Run commands such as <code>show spacefill, all</code>.</p>
          <p><strong className="text-fd-foreground">Local only.</strong> Reloading or leaving the page discards the selected file and scene.</p>
        </div>
      </main>
    </DocsLayout>
  );
}
