'use client';

import { useEffect, useRef } from 'react';

type DemoMount = (root: HTMLDivElement) => () => void;
type DemoMountHandler = (mount: DemoMount) => void;

export function LiveDemo() {
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let unmount: (() => void) | undefined;
    const key = `__molgfxMount${crypto.randomUUID().replaceAll('-', '')}`;
    const browserWindow = window as typeof window & Record<string, DemoMountHandler | undefined>;
    browserWindow[key] = (mount) => {
      if (root.current) unmount = mount(root.current);
    };

    const script = document.createElement('script');
    script.type = 'module';
    script.textContent = `import { mount } from '/molgfx/demo/workbench.js'; const handler = window[${JSON.stringify(key)}]; if (typeof handler === 'function') handler(mount);`;
    document.head.append(script);

    return () => {
      delete browserWindow[key];
      script.remove();
      unmount?.();
    };
  }, []);

  return <div ref={root} aria-label="MolGFX local-file Workbench demo" />;
}
