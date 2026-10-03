"use client";
import { createElement, useEffect, useRef, useState } from "react";
import { Viewer } from "../viewer.js";
import type { CSSProperties, ReactElement, RefObject } from "react";
import type { ViewerOptions } from "../types.js";
/** Owns exactly one SDK viewer for the mounted element, including late creation. */
export function useViewer(
  element: RefObject<HTMLElement | null>,
  options: ViewerOptions = {},
): Viewer | undefined {
  const latest = useRef(options);
  latest.current = options;
  const active = useRef<Viewer | undefined>(undefined);
  const [state, setState] = useState<{
    viewer: Viewer;
    owner: RefObject<HTMLElement | null>;
    signal: AbortSignal | undefined;
  }>();
  // A changed owner/signal is obsolete already during render, before effects run.
  const viewer =
    state?.owner === element &&
    state.signal === options.signal &&
    !options.signal?.aborted
      ? state.viewer
      : undefined;
  useEffect(() => {
    setState(undefined);
    const el = element.current;
    if (!el) return;
    const abort = new AbortController();
    const external = options.signal;
    let mounted = true,
      owned: Viewer | undefined;
    const cancel = () => {
      active.current = undefined;
      if (mounted) setState(undefined);
      abort.abort();
    };
    external?.addEventListener("abort", cancel, { once: true });
    if (external?.aborted) cancel();
    void Viewer.create(el, {
      signal: abort.signal,
      onError: (error) => {
        if (mounted && !abort.signal.aborted) latest.current.onError?.(error);
      },
      onCamera: (camera) => {
        if (mounted && !abort.signal.aborted) latest.current.onCamera?.(camera);
      },
      onPick: (event) => {
        if (mounted && !abort.signal.aborted) latest.current.onPick?.(event);
      },
      onSelectionChange: (event) => {
        if (mounted && !abort.signal.aborted)
          latest.current.onSelectionChange?.(event);
      },
    }).then(
      (next) => {
        if (!mounted || abort.signal.aborted) {
          void next.dispose();
          return;
        }
        owned = next;
        active.current = next;
        setState({ viewer: next, owner: element, signal: external });
      },
      (error: unknown) => {
        // create reports genuine failures through onError; cancellation is expected.
        if (
          mounted &&
          !(error instanceof DOMException && error.name === "AbortError")
        )
          setState(undefined);
      },
    );
    return () => {
      mounted = false;
      if (active.current === owned) active.current = undefined;
      external?.removeEventListener("abort", cancel);
      abort.abort();
      void owned?.dispose();
    };
  }, [element, options.signal]);
  useEffect(() => {
    if (!viewer || viewer !== active.current || options.structure === undefined)
      return;
    void viewer
      .load(
        options.structure,
        options.name === undefined ? {} : { name: options.name },
      )
      .catch(() => {
        // load reports its error through the SDK subscription; this handles rejection.
      });
  }, [viewer, options.structure, options.name]);
  return viewer;
}
export interface MolGFXViewerProps extends ViewerOptions {
  className?: string;
  style?: CSSProperties;
  onReady?: (viewer: Viewer) => void;
}
/** Lifecycle-only React adapter. Scene state and rendering stay in Viewer/Session. */
export function MolGFXViewer(props: MolGFXViewerProps): ReactElement {
  const element = useRef<HTMLDivElement>(null);
  const viewer = useViewer(element, props);
  const ready = useRef(props.onReady);
  ready.current = props.onReady;
  useEffect(() => {
    if (viewer) ready.current?.(viewer);
  }, [viewer]);
  return createElement("div", {
    ref: element,
    className: props.className,
    style: props.style,
  });
}
