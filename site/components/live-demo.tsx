"use client";

import { useEffect, useRef, useState } from "react";
import { Viewer } from "molgfx";
import "molgfx/viewer.css";

export function LiveDemo() {
  const root = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string>();
  useEffect(() => {
    let viewer: Viewer | undefined;
    let cancelled = false;
    const abort = new AbortController();
    async function mount() {
      try {
        if (cancelled || !root.current) return;
        viewer = await Viewer.create(root.current, {
          onError: (cause: unknown) => {
            if (!cancelled) setError(String(cause));
          },
        });
        if (cancelled) {
          await viewer.dispose();
          return;
        }
        const response = await fetch(
          "https://files.rcsb.org/download/4HHB.cif",
          { signal: abort.signal },
        );
        if (!response.ok)
          throw new Error("Structure download failed: " + response.status);
        const bytes = await response.arrayBuffer();
        if (cancelled) return;
        await viewer.load(bytes, { name: "4hhb.cif" });
        if (cancelled) return;
        viewer.execute(
          "show cartoon color=chain, protein; show spacefill color=orange, resname HEM; focus all",
        );
      } catch (cause) {
        if (!cancelled) setError(String(cause));
      }
    }
    void mount();
    return () => {
      cancelled = true;
      abort.abort();
      if (viewer) void viewer.dispose();
    };
  }, []);
  return (
    <div>
      <div
        ref={root}
        aria-label="MolGFX interactive molecular canvas"
        style={{ height: 420 }}
      />
      {error && <p role="alert">{error}</p>}
    </div>
  );
}
