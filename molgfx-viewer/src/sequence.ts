/** Typed sequence-panel state derived from renderer residue metadata. */
export interface ResidueMetadata {
  chain: string | null;
  auth_chain: string | null;
  entity: number | null;
  one_letter: string | null;
  component: string | null;
  auth_component: string | null;
  auth_number: number | null;
  label_number: number | null;
  insertion_code: string | null;
  observed: boolean;
  /** Absent for canonical gaps and for observed residues without a source mapping. */
  residue_index?: number | null;
  atom_range: [number, number] | null;
  /** Canonical position is optional for legacy scene metadata. It is never inferred. */
  canonical_position?: number | null;
}

export interface SequenceResidue {
  readonly key: string;
  readonly residueIndex: number | null;
  readonly chain: string | null;
  readonly authChain: string | null;
  readonly entity: number | null;
  readonly oneLetter: string | null;
  readonly component: string | null;
  readonly authComponent: string | null;
  readonly authNumber: number | null;
  readonly labelNumber: number | null;
  readonly insertionCode: string | null;
  readonly observed: boolean;
  readonly atomRange: readonly [number, number] | null;
  readonly canonicalPosition: number | null;
  readonly isGap: boolean;
}

export interface SequenceChain {
  readonly key: string;
  readonly label: string;
  readonly authLabel: string | null;
  readonly entity: number | null;
  readonly residues: readonly SequenceResidue[];
  readonly hasOneLetter: boolean;
  readonly oneLetter: string | null;
}

export interface SequencePanelModel {
  readonly chains: readonly SequenceChain[];
  readonly selectedChain: string | null;
}

function chainKey(item: ResidueMetadata): string {
  return [item.chain ?? "", item.auth_chain ?? "", item.entity ?? ""].join("\u0000");
}

function residueKey(item: ResidueMetadata, ordinal: number): string {
  const range = item.atom_range;
  if (range) return `atom-range:${range[0]}-${range[1]}`;
  if (item.residue_index !== null && item.residue_index !== undefined) return `residue-index:${item.residue_index}`;
  if (item.canonical_position !== null && item.canonical_position !== undefined) return `canonical-gap:${item.canonical_position}`;
  return `unmapped:${ordinal}`;
}

export function residueNumberLabel(residue: SequenceResidue): string {
  const number = residue.authNumber ?? residue.labelNumber;
  return (number === null ? "?" : String(number)) + (residue.insertionCode ?? "");
}

export function buildSequencePanelModel(metadata: readonly ResidueMetadata[], selectedChain?: string | null): SequencePanelModel {
  const chains = new Map<string, SequenceResidue[]>();
  const descriptors = new Map<string, ResidueMetadata>();
  metadata.forEach((item, ordinal) => {
    const key = chainKey(item);
    const residues = chains.get(key) ?? [];
    const residueIndex = item.residue_index ?? null;
    residues.push({
      key: residueKey(item, ordinal), residueIndex, chain: item.chain, authChain: item.auth_chain,
      entity: item.entity, oneLetter: item.one_letter, component: item.component,
      authComponent: item.auth_component, authNumber: item.auth_number, labelNumber: item.label_number,
      insertionCode: item.insertion_code, observed: item.observed, atomRange: item.atom_range,
      canonicalPosition: item.canonical_position ?? null,
      isGap: residueIndex === null && item.atom_range === null,
    });
    chains.set(key, residues);
    descriptors.set(key, item);
  });
  const result = [...chains].map(([key, residues]) => {
    const first = descriptors.get(key)!;
    const letters = residues.map((residue) => residue.oneLetter);
    const hasOneLetter = letters.some((letter) => letter !== null && letter !== "");
    return {
      key, label: first.chain ?? first.auth_chain ?? "chain", authLabel: first.auth_chain,
      entity: first.entity, residues, hasOneLetter,
      oneLetter: hasOneLetter ? letters.map((letter) => letter ?? "?").join("") : null,
    };
  });
  return {
    chains: result,
    selectedChain: selectedChain && result.some((chain) => chain.key === selectedChain)
      ? selectedChain : result[0]?.key ?? null,
  };
}

export function selectSequenceChain(model: SequencePanelModel, key: string): SequencePanelModel {
  return model.chains.some((chain) => chain.key === key) ? { ...model, selectedChain: key } : model;
}

export function wrappedOneLetter(chain: SequenceChain, width = 80): readonly string[] {
  if (!chain.hasOneLetter || !chain.oneLetter || width < 1) return [];
  const rows: string[] = [];
  for (let offset = 0; offset < chain.oneLetter.length; offset += width) rows.push(chain.oneLetter.slice(offset, offset + width));
  return rows;
}

/** The renderer already provides canonical order; this helper keeps gaps visible. */
export function displayedResidues(chain: SequenceChain): readonly SequenceResidue[] {
  return chain.residues;
}

/** Converts selected observed residues into sorted, compact inclusive intervals. */
export function residueIntervals(residues: readonly SequenceResidue[]): [number, number][] {
  const indices = [...new Set(residues.flatMap((residue) => residue.residueIndex === null ? [] : [residue.residueIndex]))].sort((a, b) => a - b);
  const intervals: [number, number][] = [];
  for (const index of indices) {
    const previous = intervals[intervals.length - 1];
    if (previous && index <= previous[1] + 1) previous[1] = index;
    else intervals.push([index, index]);
  }
  return intervals;
}

export function metadataFromScene(scene: { residueMetadataJSON(structure: number): string } | { residueMetadataJSON(structure: bigint): string }, structure: number | bigint = 1): ResidueMetadata[] { const read = scene.residueMetadataJSON as (value: number | bigint) => string; return JSON.parse(read.call(scene, structure)) as ResidueMetadata[]; }

export interface SequencePanelOptions {
  metadata: readonly ResidueMetadata[];
  onResidueClick?: (residue: SequenceResidue, event: MouseEvent) => void;
}

/** Mounts an accessible sequence view. Gaps are inert and never yield atom selections. */
export function mountSequencePanel(parent: HTMLElement, options: SequencePanelOptions): { update: (metadata: readonly ResidueMetadata[]) => void; dispose: () => void } {
  const root = document.createElement("section");
  root.className = "molgfx-sequence-panel";
  root.setAttribute("aria-label", "Sequence");
  parent.appendChild(root);
  const render = (metadata: readonly ResidueMetadata[]) => {
    root.replaceChildren();
    const model = buildSequencePanelModel(metadata);
    if (model.chains.length === 0) return;
    const firstChain = model.chains[0];
    if (!firstChain) return;
    const chooser = document.createElement("select");
    chooser.setAttribute("aria-label", "Chain");
    for (const chain of model.chains) {
      const option = document.createElement("option"); option.value = chain.key; option.textContent = chain.label; chooser.appendChild(option);
    }
    root.appendChild(chooser);
    const body = document.createElement("div"); root.appendChild(body);
    const paint = (key: string) => {
      body.replaceChildren();
      const chain = model.chains.find((candidate) => candidate.key === key);
      if (!chain) return;
      for (const residue of chain.residues) {
        const button = document.createElement("button"); button.type = "button"; button.disabled = residue.isGap;
        button.className = residue.isGap ? "is-gap" : ""; button.textContent = residue.oneLetter ?? residue.component ?? "·";
        button.title = residueNumberLabel(residue);
        button.addEventListener("click", (event) => options.onResidueClick?.(residue, event)); body.appendChild(button);
      }
    };
    chooser.addEventListener("change", () => paint(chooser.value)); chooser.value = model.selectedChain ?? firstChain.key; paint(chooser.value);
  };
  render(options.metadata);
  return { update: render, dispose: () => root.remove() };
}
