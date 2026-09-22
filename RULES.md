# Rules

House rules for code in this repository. They are not style preferences with
exceptions — they are constraints, and the build is configured to keep them.

This covers *how* the code is written. A renderer lives or dies on the inner loop
and on the frame budget, so several of these rules are sharper here than they would
be in an ordinary library.

---

## 1. Comments and docstrings never cite the specification

No requirement numbers, no document names, no section references, no ADR
numbers — nothing that points at a document living outside the code. Not
`FR-206`, not `04-gpu-data-layout.md §3`, not `ADR-0003`.

Explain the reasoning in its own terms instead. Docstrings should still be
thorough — the rule removes a pointer, not the explanation it pointed at.

**Why.** The specification is a separate artefact with its own lifecycle. It
will be renumbered, split and rewritten, and every citation in the code becomes
a lie the moment it is. A comment that says *why* stays true; a comment that
says *where it was decided* does not.

**In practice.**

```rust
// Wrong — cites the spec.
/// Impostors stay meshless (ADR-0003, see 09-representations.md §2).

// Right — says the thing.
/// A sphere is drawn as two triangles and intersected analytically in the
/// fragment shader. There is no tessellated mesh, so the silhouette is exact
/// at any zoom and the per-atom memory cost is a point, not a vertex ring.
```

Write the docstring as if the specification did not exist and the reader has only
the code in front of them.

---

## 2. Files cap at roughly 400–500 lines

When a module approaches the cap, split it into a directory module before adding
more. Do not let one file grow past it and plan to tidy later.

**Why.** A file that outgrows a screenful of structure has usually outgrown its
single responsibility too. The cap forces the split at the point where the seam
is still obvious. A render pass, its pipeline, its bind-group layout and its
resource declarations are four responsibilities, not one file.

**In practice.** `foo.rs` becomes `foo/` with `mod.rs` plus one file per
responsibility. A WGSL file counts too: split a growing shader into shared
includes in `molgfx-shaders` rather than one thousand-line source.

---

## 3. No `unwrap`, `expect`, or any `unwrap_*` variant outside tests

This includes `unwrap_or`, `unwrap_or_default` and `unwrap_or_else`. Handle
absence with an explicit `match` or `let ... else`.

**Why.** Writing the absent case out forces you to decide what it means *here*,
and puts that decision where a reader will find it. On a renderer the absent case
is usually a lost device, an unsupported feature or a surface that went stale
between frames — silently defaulting any of those produces a black screen with no
explanation. Device loss is a state to handle, not a panic.

**In practice.**

```rust
// Wrong.
let frame = surface.get_current_texture().unwrap();

// Right — the recoverable case is now visible and named.
let frame = match surface.get_current_texture() {
    Ok(frame) => frame,
    Err(SurfaceError::Lost | SurfaceError::Outdated) => {
        self.reconfigure();
        return Ok(FrameOutcome::Skipped);
    }
    Err(e) => return Err(RenderError::Surface(e)),
};
```

No Clippy warning is silenced to preserve this rule. Write the branch explicitly
in a shape that is both clear and lint-clean. Tests are exempt from the panic
restriction: prefer `let ... else { panic!("...") }` there so the failure
message says what was expected.

---

## 4. Nothing allocates per atom, per frame, or per iteration

Reach for a pre-sized pool, an index or a borrowed slice before a `Vec` or a
`String`. A GPU buffer is allocated once and reused; it is never freed and
reallocated inside the frame loop.

**Why.** The engine's entire claim is that biology-scale structures render at
interactive rates. One heap allocation on the per-atom upload path or one buffer
recreation per frame forfeits that claim regardless of how correct the picture
is. Allocation counts are gated rather than wall time because they do not move
with the machine or the clock.

**In practice.**

- Interned `u32` identifiers, never strings, in anything a shader or a pack loop
  touches. The dictionary comes from `molframe`; carry the ids through unchanged.
- One staging arena reused across frames, not one allocation per upload.
- A reserved sentinel value instead of `Option<T>` on a per-atom column.
- Pool transient GPU textures through the render graph; do not create a
  depth buffer every frame.
- Compare squared distances against a squared cutoff; take the square root only
  when a caller wants a length.
- Read the data-layout reasoning in the specification before inventing a new
  buffer format. The SoA packing, the alignment and the zero-copy seam are there
  for reasons that are written down.

---

## 5. Minimal asymptotic cost, and say what it is

Choose the algorithm whose growth matches the workload, and state the cost in the
module docstring where it is not obvious. State the per-frame GPU cost too where
a pass scales with atom count, pixel count or light count.

**Why.** A quadratic neighbour scan is invisible on a single ligand and fatal on
a ribosome; a full-resolution shadow pass is invisible at 1080p and fatal at 4K.
Stating the cost makes a regression something a reviewer catches by reading
rather than by profiling on the one machine that shows it.

**In practice.** Prefer a BVH query over an all-pairs scan; prefer indirect,
GPU-driven draw over a CPU loop that issues one call per atom; use per-chunk
bounding boxes and element masks to skip whole regions before touching a row.
Where a pass is `O(pixels × lights)` or `O(atoms × samples)`, write it down next
to the pipeline that pays it.

---

## 6. `lib.rs` and `mod.rs` contain no implementation

Only `mod` declarations, `pub use` re-exports, and the module docstring. A
`#[cfg(test)] mod fixture;` declaration is fine.

**Why.** It makes the shape of a crate readable in one file, and it means moving
an item between modules never touches the file that publishes it. The facade
crate `molgfx` takes this furthest: it is re-exports and feature gates only.

The facade publishes only the stable declarative vocabulary and curated
namespaces. It never re-exports whole implementation crates and never exposes a
`Deref` route into renderer internals.

---

## 7. Tests live in a sibling file

Declared from the source file:

```rust
#[cfg(test)]
#[path = "thing_tests.rs"]
mod tests;
```

so `thing.rs` is paired with `thing_tests.rs`. Test names are sentences that
state the expected behaviour, so a failure is readable without opening the file:

```rust
#[test]
fn a_capsule_impostor_bounds_both_endpoint_spheres() { ... }
```

Golden-image tests name their reference scene and their tolerance in the test
name and body; a perceptual diff that exceeds tolerance writes the diff image to
the target directory rather than only asserting.

---

## 8. Deterministic, complete, and DRY

**Deterministic.** The same input and the same camera produce the same frame on
the named reference adapter, down to the bytes, and the same analysis produces
the same ordering of findings — with no dependence on hash iteration order,
thread count, or the order the GPU retired its work. Anything that sorts or
fingerprints does so on a stable key. Floating-point non-determinism across
drivers is real; the determinism contract binds a named adapter, and everything
above the shader is exactly reproducible.

**Complete.** Prefer capabilities that can be finished and verified over ones
that need hardware or data not present here. A realtime path that compiles,
renders and passes its golden scenes is worth more than three half-written
ray-tracing experiments.

**DRY.** One implementation per idea, but extract the *idea*, not a superficial
similarity. Sphere and capsule impostors share one analytic-intersection
scaffold because they genuinely want the same ray-primitive machinery; two passes
that both happen to clear a texture do not.

---

## 9. `unsafe` is confined to one audited boundary

`#![forbid(unsafe_code)]` is the default in every crate. Exactly one boundary is
permitted `unsafe`, and only for its stated reason:

- **`bytemuck` POD casts** — reinterpreting a packed `#[repr(C)]` vertex/atom
  struct as bytes for GPU upload. Use the `bytemuck` derives, never a hand-rolled
  `transmute`.

`unsafe` appearing anywhere else is a bug, not a shortcut.

**Why.** wgpu already gives a safe GPU abstraction; the only genuine `unsafe` left
is byte reinterpretation. Confining it to one named boundary means the audit
surface is a grep, not a codebase.

---

## 10. No per-atom CPU draw calls — the GPU draws itself

The CPU records the frame; it does not iterate atoms. Draw impostors instanced,
and drive large scenes with indirect draw whose counts are produced by a compute
culling pass. There is no code path that issues one draw per atom, per bond or
per residue.

**Why.** A million atoms at one draw call each is a million CPU submissions and a
dead frame budget. Instanced and indirect draw move the per-primitive work onto
the GPU where it belongs, and they are the reason the billion-atom ceiling is
even discussable.

**In practice.** An atom is one instance of a two-triangle quad; a bond is one
instance of a capsule quad. Visibility, LOD selection and indirect-count
generation happen in compute shaders that write `DrawIndirect` args the CPU never
inspects per atom.

---

## 11. Shaders are disciplined the way code is

WGSL is source, not scratch. Every bind-group layout is documented where it is
declared; every binding index has a name and a reason. No magic constants in a
shader body — probe radii, cutoffs and sample counts arrive as named uniforms or
`override` constants, never as a literal `1.4` buried in a function.

**Why.** A shader is the hottest code in the program and the least visible in a
stack trace. The same readability and single-source rules that keep Rust
maintainable keep WGSL debuggable; a shared intersection routine in
`molgfx-shaders` is included by both the realtime and quality paths so the two
never drift.

---

## Checking

Everything above is checked by:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings   # zero warnings
cargo fmt --all --check
cargo test --workspace --all-features
```

plus these greps, each of which must come back empty:

```bash
# No unwrap/expect/unwrap_* outside test files. Tests live in `*_tests.rs`,
# in `tests.rs` and under `generic_tests/`, hence the three exclusions.
grep -rn "unwrap" crates/ --include="*.rs" | grep -v "_tests.rs" | grep -v "/tests.rs" | grep -v "generic_tests/"

# No source or manifest lint suppression. Fix every warning at its cause.
grep -rnE '#\[(allow|expect)\b' crates/ --include="*.rs"
grep -rnE 'level[[:space:]]*=[[:space:]]*"allow"|allow[[:space:]]*=' --include="*.toml" .

# The file cap, Rust and WGSL alike.
find crates \( -name "*.rs" -o -name "*.wgsl" \) -print0 \
  | xargs -0 wc -l | awk '$1>500 && $2 != "total" {print}'

# The audit surface: `unsafe` as a keyword, so `forbid(unsafe_code)` and prose
# mentions do not register. Only `bytemuck` casts are permitted.
grep -rnE "(^|[^A-Za-z_])unsafe([[:space:]]*\{|[[:space:]]+(fn|impl|trait|extern|static|mut))" \
  crates/ --include="*.rs" | grep -v bytemuck
```

No check reads a scene corpus, so a checkout with no `benchmarks/` passes the
whole list; the benches that want data resolve it through
`crates/molgfx-bench/src/fixtures.rs` and skip when it is absent. That makes the
seam an invariant:

```bash
# Names a corpus directory in exactly one place — the benchmark harness's
# resolver. Any other hit means a call site built a path itself.
grep -rn "benchmarks/" crates/ --include="*.rs"
```

The Python binding is checked against runtime behavior and its precise stubs:

```bash
maturin develop -m crates/molgfx-py/Cargo.toml
python -m unittest discover -s python/tests
python -m mypy.stubtest molgfx._engine
```

`mypy.stubtest` runs with no allowlist. Anything it cannot express is fixed in
the stub or in the binding, not excused. Repository policies such as line count,
lint suppression, and unsafe boundaries are workflow shell checks, never product
tests that inspect source text.
