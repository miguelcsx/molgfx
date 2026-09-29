'use client';

import { useEffect, useRef } from 'react';

// The same viewer the Jupyter widget runs, built once by
// crates/molgfx-wasm/js and copied into public/runtime/.
const RUNTIME = '/molgfx/runtime';

// A short helix that stands in when the archive is unreachable, so the demo
// still starts offline.
const examplePdb = [
  'ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 20.00           N',
  'ATOM      2  CA  ALA A   1       1.458   0.000   0.000  1.00 20.00           C',
  'ATOM      3  C   ALA A   1       2.008   1.410   0.000  1.00 20.00           C',
  'ATOM      4  N   ALA A   2       3.358   1.640   0.420  1.00 20.00           N',
  'ATOM      5  CA  ALA A   2       3.988   2.964   0.510  1.00 20.00           C',
  'ATOM      6  C   ALA A   2       3.220   3.880   1.470  1.00 20.00           C',
  'ATOM      7  N   ALA A   3       3.862   5.070   1.810  1.00 20.00           N',
  'ATOM      8  CA  ALA A   3       3.260   6.018   2.760  1.00 20.00           C',
  'ATOM      9  C   ALA A   3       1.816   6.330   2.390  1.00 20.00           C',
  'ATOM     10  N   ALA A   4       1.340   7.606   2.470  1.00 20.00           N',
  'ATOM     11  CA  ALA A   4      -0.046   7.932   2.170  1.00 20.00           C',
  'ATOM     12  C   ALA A   4      -0.836   6.878   1.390  1.00 20.00           C',
  'ATOM     13  N   ALA A   5      -2.168   7.150   1.170  1.00 20.00           N',
  'ATOM     14  CA  ALA A   5      -3.038   6.194   0.460  1.00 20.00           C',
  'ATOM     15  C   ALA A   5      -2.538   4.765   0.740  1.00 20.00           C',
  'ATOM     16  N   ALA A   6      -3.492   3.900   0.280  1.00 20.00           N',
  'ATOM     17  CA  ALA A   6      -3.136   2.520   0.290  1.00 20.00           C',
  'ATOM     18  C   ALA A   6      -1.750   2.218  -0.270  1.00 20.00           C',
  'ATOM     19  N   ALA A   7      -1.430   0.900  -0.400  1.00 20.00           N',
  'ATOM     20  CA  ALA A   7      -0.120   0.508  -0.910  1.00 20.00           C',
  'ATOM     21  C   ALA A   7       0.640   1.624  -1.580  1.00 20.00           C',
  'TER',
  'END',
].join('\n');

const options = {
  // Oxyhaemoglobin, fetched from the RCSB by the visitor's browser. Four haem
  // groups, four chains: enough to show cartoon, surface, ligand and labelling
  // at once.
  structure: { name: '1hho.cif', url: 'https://files.rcsb.org/download/1HHO.cif' },
  fallback: { name: 'alanine-helix.pdb', data: examplePdb },
  // One program shows the engine's range: a query-driven pocket, four
  // representations combined on the same scene, chain colouring of the pocket,
  // a label and a framed focus.
  program: [
    'select heme, resname HEM',
    'select pocket, byres (within 5 of $heme) and protein',
    'show cartoon color=chain, protein',
    'show surface style=soft_union opacity=0.35, protein',
    'show ball_and_stick radius=0.25 as pocket, $pocket',
    'show spacefill color=orange, $heme',
    'label "heme", $heme',
    'focus $heme',
  ].join('; '),
};

type Mount = (root: HTMLElement, mountOptions: typeof options) => () => void;

export function LiveDemo() {
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let unmount: (() => void) | undefined;
    let cancelled = false;
    const stylesheet = document.createElement('link');
    stylesheet.rel = 'stylesheet';
    stylesheet.href = `${RUNTIME}/widget.css`;
    document.head.append(stylesheet);

    // A runtime URL, not a bundled module: the wasm glue is loaded beside it.
    import(/* webpackIgnore: true */ /* turbopackIgnore: true */ `${RUNTIME}/mount.js`).then(({ mount }: { mount: Mount }) => {
      if (!cancelled && root.current) unmount = mount(root.current, options);
    });

    return () => {
      cancelled = true;
      unmount?.();
      stylesheet.remove();
    };
  }, []);

  return <div ref={root} aria-label="MolGFX live Workbench" />;
}
