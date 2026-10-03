# Plan integral: MolGFX frente a Mol*/PyMOL y MolFrame frente a Gemmi

## Contexto y decisiones

Completar las diferencias verificadas de `molgfx/mapping.md` y `molgfx/missing.md` frente a los checkouts locales de Mol* y PyMOL, incluyendo las fases A–E y todas las filas de representaciones, colour/size, rendering, interacción y exportación que pertenecen al engine. Las casillas del inventario no sustituyen ejecución. Preservar el trabajo sin sincronizar; código declarativo, una autoridad por política y ningún análisis molecular duplicado en el viewer.

Decisiones del usuario: paridad de librerías, no reconstruir aplicaciones; 120 FPS **también a máxima calidad**, sin reducir resolución, muestras o detalle para aprobar; MolGFX autónomo con dependencia MolFrame fijada por Git; trabajar en main y hacer commits/push ordinarios sólo por unidades listas, probadas, limpias de Clippy y benchmarkeadas. No publicar paquetes, crear ramas, reescribir historia ni forzar pushes. «Mejor que Unreal» queda como aspiración: no es una afirmación verificable sin una escena, hardware y receta comparable.

Este documento conserva el alcance completo acordado. Los verbos de implementación describen objetivos, no resultados certificados. Estado y evidencia actualizados: [mapping.md](mapping.md) y [missing.md](missing.md). Los checks históricos no certifican cambios posteriores.

## Hallazgos de la auditoría inicial (históricos, no estado Git actual)

- MolGFX: main, HEAD d5c9ee8; 47 archivos rastreados modificados, dos nuevos (`overlay/lower_guides.rs` y `overlay/planes.rs`), nada staged ni commits por delante de origin/main. Contiene trabajo de planos/celda/undo y payload flat de impostores cerca del near plane; no revertirlo.
- MolFrame: main limpio en 269b979; Mol* 0efa4dfa5; PyMOL 5e8bfca5. El pin MolFrame ae827fe2f01d98adbf0b3d360bc69bf3567bf950 está dos commits detrás del vecino. `molgfx/Cargo.toml` usa Git, **no** el path patch descrito por memoria antigua; el checkout vecino no entra automáticamente en MolGFX.
- `mapping.md`/`missing.md` se contradicen sobre guías planas, celda, selección y temas. El número «once estados DSSP» mezcla estados estructurales con DNA/RNA/carbohidratos; la tabla de colores y las granularidades también deben compararse por conjuntos reales, no por cifras antiguas.
- La química, conectividad estándar, precedencia de enlaces de archivo, percepción explícita y fallback de SS ya existen en MolFrame. MolGFX conserva enlaces/SS, pero cuantiza Quadruple a tres strands. `AssemblySpec.instances` conserva matrices affine pero no se baja a placements moleculares; `RigidInstance` no puede representar shear/nonuniform scale.
- Hay flechas de strand, slabs/rings nucleicos, arcos de medidas, volúmenes y segmentación internos. Los gaps son fidelidad/controles/autoría pública, no ausencia total. Dots baja a Points y no cumple puntos expuestos vdW/SAS. Labels ignora color y fuerza mayúsculas.
- `ScalarVolume` ya acepta affine y MolFrame ya parsea MRC. La fachada de volumen origin/spacing pierde skew y fuerza isosurface; falta exponer estilos/slices/segmentación y escenas sólo volumen.
- Picking detached ya existe, pero `Picker.submission` es mutable y un segundo pick puede sustituir la procedencia del primero. El viewer bloquea dibujo durante readback y valida sceneEpoch sin toda la identidad de vista. La selección textual resid/chain pierde estructura, inserción e instancia.
- Measurement pick devuelve value=None. Volume-segment pick descarta el handle y elige `volumes.first_key_value()`: incorrecto con más de un volumen. Scalar VolumeHandle y SegmentationHandle no son intercambiables.
- Existen dos SceneSnapshot y dos MovieExportRequest. SetSnapshot retiene JSON en extensions, no restaura estado vivo. CameraPath/Bookmark/Timeline/TimeWarp ya existen y se deben publicar/reutilizar.
- `render_sequence` promete convergencia completa; `image.rs` asigna SequenceFrame una muestra. `frame_time.rs` divide publicación por 64 sin observar su presupuesto; el batch profiler conserva sólo la última pareja de timestamps. Ninguno prueba 120 FPS a máxima calidad.
- Los 3,4/9,1/31,6/138 ms del ledger son históricos y exclusivos de MolGFX. StatsAlloc/live bytes no son RSS; allocation_events de residency no son todas las allocations del heap. Pedir spacing 0,25 Å tampoco garantiza spacing efectivo: el cap 192 coarsens campos grandes.
- Referencia física: Apple M5 Pro CPU18/GPU20, RAM24GB; ASUS PA279CV 3840×2160, UI1920×1080, refresh30Hz. Throughput offscreen no prueba presentación a 120Hz.

## Contratos transversales y límites de arquitectura

1. MolFrame posee parsing, química, selección, SS, carbohidratos, geometría analítica de análisis y propiedades moleculares. MolGFX posee autoría, placements, geometría renderizable, GPU, interacción y exportación. El consumidor posee ventana, fetch/decode de assets, permisos XR y codificación de vídeo. Bindings dependen sólo de la fachada MolGFX.
2. Coordenadas prestadas/Arc y upload directo; una topología fuente y muchas colocaciones. Nada de materializar assemblies, clonar posiciones por átomo o draws por entidad. Átomos/enlaces siguen impostores analíticos; teselación sólo en frontera de exportación.
3. Un spec tipado por dominio; constructor y lowering comparten validación. Patches validadas antes de mutar e inverse completo; errores explícitos por fuente ausente, formato, stale generation, presupuesto, capacidad o pérdida de dispositivo. Ninguna geometría vacía, primer elemento o fallback silencioso para ocultar un incumplimiento.
4. Cutover limpio de contratos: migrar Rust/command/Python/WASM, serializers, stubs, ejemplos y tests afectados en la misma unidad. No aliases deprecated, schema versions manuales ni segundo registro de visual inputs. Reusar registry/VM, caches, uploader y métricas existentes.
5. Identidad canónica colocada = StructureId + instance opcional + source row + topology/content revision. Usar un solo nombre de campo `instance`; posiciones públicas explícitas source_position/world_position. Enlaces pueden tener endpoints de placements distintos.
6. Segmentación declarativa usa **SegmentationId**, descriptor/binding categorical y SegmentationHandle generacional; VolumeId queda para density scalar. La resolución de picks categorical devuelve SegmentationId+label, no un VolumeId arbitrario. Ambas rutas conservan source_id y generación en la captura detached.
7. Un solo presupuesto de exposición resuelto por perfil. SequenceConfig añade `SequenceExposure::{Converged, Progressive}`; Converged es el contrato por defecto de la fachada. Progressive es explícito y nunca cuenta como máxima calidad. No crear a la vez SequenceQuality y otra policy de samples.
8. Una máscara/compositor de markers. Orden final: iluminación/transparencia → acumulación temporal/DOF/motion blur/bloom del mundo → tonemap → markers actuales visibles/ghost → AA de presentación → CAS → overlays de pantalla. Ghost nunca modifica depth/IDs ni entra en historia. Tint material que cambia radiancia sí invalida ésta; edge-only no.
9. No language servers configurados durante la auditoría. En ejecución, usar LSP references si está disponible antes de cambiar símbolos exportados; en su ausencia AST/grep. Respetar RULES: módulos cohesionados <=500 líneas, lib/mod sólo declaraciones/reexports, tests hermanos, sin unwrap*/expect fuera de tests, sin lint suppressions ni unsafe nuevo.

## Secuencia de implementación

### 1. Trabajo local, integración autónoma y gate de Git

Revisar primero el diff existente y continuar sus guías/near-plane, sin sustituir PlaneSpec/lower_planes/lower_unit_cells por APIs duplicadas. Probar plano degenerado, estructura desconocida, plano/celda skew, add/remove/undo y cámara dentro/cerca de esfera/cápsula: errores tipados o imagen/pick correcto, nunca triángulos negros.

Mantener pins Git publicables. Para desarrollo coordinado, preparar CARGO_HOME temporal fuera de ambos árboles con config.toml que parchee molframe de crates.io y del Git miguelcsx/molframe, y molframe-py del mismo Git, a rutas absolutas del vecino; reutilizar sus caches registry/git. Cargo Book documenta la precedencia de config patch sobre manifest patch. Propagar ese entorno a Rust/maturin/wasm-pack/build.mjs. Cargo metadata debe demostrar una sola identidad local por paquete; no editar versiones ni publicar pins locales.

Por cada unidad MolFrame completa: smoke + suite/Clippy + benchmark del camino tocado → commit firmado/sign-off → push ordinario main. Actualizar en MolGFX `workspace.dependencies.molframe-py.rev` y `patch.crates-io.molframe.rev` al SHA completo **ya publicado**, resolver lockfile con Cargo y verificar sin overrides antes de sincronizar MolGFX. Unidades MolGFX completas siguen el mismo gate. Orígenes: git@github.com:miguelcsx/molframe y git@github.com:miguelcsx/molgfx.

Conventional Commits/scopes de AGENTS, cuerpo en bullets <=72 columnas, `git commit -S -s`, sin Co-authored-by. Objetivo <=400 líneas fuente cuando la separación sea coherente. Incluir el trabajo local preservado sólo después de revisarlo/verificarlo. Si origin avanzó, inspeccionar divergencia sin reset/rebase/amend/force-push; obtener autorización para resolverla. No sincronizar estado conocido como incompleto.

### 2. Medición verdadera y baseline antes de optimizar

Extender `Quality` existente con HighestFixed y trasladar implementación fuera de profile/mod.rs. Fixed tier High permanece fijo aunque disabled/adaptive=false, incluso con millones de átomos; target_fps es objetivo, no permiso para degradar. Converged/HighestFixed comparten detalle máximo declarado. Añadir metadata observable al report/image/profiler: extent, tier, samples required/submitted/completed, spacing solicitado/efectivo, ribbon steps, AO/illumination, bounces, LOD, adaptation/degradation, full residency y complete.

Consolidar recorder/exposure de render/image/sequence/profile: congelar escena/tiempo/cámara por output, sync sólo por revisiones, muestras jitter en uniform arena con offsets/bind groups persistentes, history correcto por output y fence final. No escribir 64 uniforms al mismo rango antes de una submission. Un output convergido de 64 muestras cuenta **uno**. No avanzar trayectoria 64 veces ni reusar exposición vieja tras cámara/pose distinta. Bounded in-flight slots no reutilizados hasta completion.

Extender FrameTiming/GpuProfiler, no otro profiler. Medir sync/upload/record/submit/wait CPU; timestamps distintos por ocurrencia real de render/compute pass, incluidos pre-grafo, campos/erosión/normales/componentes, BVH, cull, sombras, productores opacos/translucent, AO/GI, OIT/peeling, volúmenes, labels, TAA/DOF/bloom/markers/AA/CAS y ojos. Resolve bytes en scratch reservado; conservar todos los samples/outputs, no la última query como media. Unresolved/stale/orden inválido produce null+motivo, no cero ni tiempo heredado. Completion espera fence sin readback de pixels; screenshots fuera del intervalo medido. Capability timestamps ausente no impide latency end-to-end.

Reusar molgfx-bench FrameSample::measured, CumulativeTelemetry y summarize; separar residency allocation events, heap allocations/reallocations y RSS. Corregir frame_time/resources y hacerlos consumidores del harness común. Preservar métricas nearest-rank/errores de regresión, mover implementación de lib.rs a módulo cohesivo si procede. Crear un runner parity registrado en Cargo (autobins=false), con manifest común para tres engines, no tres corpus/cámaras/summarizers.

Primera baseline: mismos bytes/hashes/licencias, cámara física/selección/radios/colores/luz/efectos explícitos y settings efectivos por engine. Confirmar disponibilidad y poblar corpus; falta de fixture no es skip verde. Incluir proteína 4HHB/1AON, DNA/RNA, ligando con órdenes/aromaticidad, glycan, assembly y density MRC skew; sintéticos 100k/500k/1M y campos analíticos reproducibles. Comparar counts/provenance/SS antes de imagen. Mol* raster/ImagePass/tracing y PyMOL raster/ray son recetas distintas y se reportan separadas, no un speedup apples-to-oranges.

### 3. MolFrame: fidelidad química, SS y políticas fuente

- Mantener enlaces de archivo primero, tablas estándar/polymer links y percepción explícita existente; añadir Zero como orden distinto de Unknown y transportar provenance. No inferir Zero de metal ni afirmar que CONECT conoce todo orden. Formatos que pierden información deben diagnosticarlo.
- Ampliar assignment SS con AlphaHelix/ThreeTenHelix/PiHelix/BetaBridge/Strand/Turn/Bend/Coil/OtherHelix/Unknown. Conservar detalle de archivo/clase helix, insertion codes/modelos y roundtrip CIF/PDB; no colapsar toda HELX a alpha. DSSP H/G/I/E/B/T/S: donantes/conformers coherentes, proline, turns/runs, bridges/ladder intercadena, breaks reales. Precedencia file > DSSP > CA-only local > Unknown; CA fallback sólo donde backbone no clasifica, no a costa de anotación de archivo.
- Separar estados SS de categorías DNA/RNA/carbohidrato del tema. Extender query Column/Macro/printing/completion para SS usando el parser MolFrame, no evaluador MolGFX.
- Clasificar candidatos de enlaces instanciados reutilizando la política química existente con thresholds/scratch preparados. AssemblyView.neighbors/collect_crystal_neighbors generan contactos por cutoff; **no** convertirlos todos en covalent bonds ni inferir entre datasets independientes.
- AtomDisplayPolicy canónica upstream: altloc existente, hydrogens All/HideAll/HideNonPolar y zero occupancy. Nonpolar según química bonded, missing occupancy no es cero; metadata insuficiente no se inventa. Effective query del render intersecta display mask y máscara de placement sin cambiar semántica de consultas fuente.
- Añadir sólo capacidades upstream faltantes: carbohydrate graph/rings/links para SNFG; variante surface_points_into con scratch compartido; principal_axes indexed/streaming si hace falta; affine MRC derivado de cell/sampling/starts/origin con parser existente; carga PEOE/propiedad y potencial electrostático si faltan en fachada. Sin duplicar parsing, SASA, eigensolvers ni fitting.

User addition: review and implement MolFrame parity with Gemmi’s strongest features. Compare local Gemmi97c808222f468f8188f2ed87266e0d7c5a854ce2 against current MolFrame, retaining existing CIF/PDB/BCIF,530Hall symmetry, crystal neighbors, map codecs/samplers and reflection I/O. First verified gap: reciprocal geometry/d-spacing and exact reflection centricity/systematic absences/epsilon, with borrowed Miller iteration, shared MTZ calculation, Rust/Python facade and actual all530-setting differential. Continue auditing density/scattering/reciprocal workflows; this first slice is not a claim of complete Gemmi parity. No Gemmi dependency or copied licensed implementation in product code.

Prueba de frontera: fixtures con órdenes/altloc/SS de archivo y backbone incompleto/intercadena; counts/estados referencia, coordenadas Arc compartidas, lectura/perceive/query y roundtrip reales. Benchmark percepción/queries/samplers añadidos antes de publicar pin.

### 4. Assemblies, symmetry y picking colocado

AssemblyInstance conserva id/StructureId/transform row-major y gana selección fuente/operator/assembly metadata. Biological/crystal/supercell usan APIs xtal y máscaras por chain instance, no materialize. Semántica: estructuras incluidas se dibujan sólo en instances declaradas; fuera de assembly conservan identity; retirar assembly restaura identity. No copia identity adicional implícita.

Resolution pasa a PlacementKey(structure,instance) y reverse-map. Una RepresentationId puede tener varios handles; appearance/hide/remove/rebind/interactions/anchors alcanzan todos. Lane affine GPU model_to_world/world_to_model con conversión row→column explícita; validar última fila affine, finitud, invertibilidad y bounds de ocho esquinas. Ray local sin normalizar dirección conserva t; normales inverse-transpose. No reducir shear/nonuniform a quaternion.

Topología colocada separada de SourceTopology: PlacedAtomRef/InstancedBond, endpoints y lane placement_a/b en BondGpu. Enlace intra-placement usa su local affine; cross-placement usa ambos endpoints world y radios world declarados. Dedup por placement+atom, clasificar candidatos con química MolFrame, conservar orden/provenance y source IDs. Cache/budget por revisión, fuera del steady draw loop; editar color no percibe enlaces de nuevo.

Atom/bond picks conservan StructureId/instance/source row/revision/operator y source/world position actual; no resolver owner como «único dataset». Anchors atom/selection aceptan instance explícita; multi-copy sin instancia necesaria es ambiguo, no primera copia. Prueba: translate/shear/nonuniform, dos chains, enlace intra/cross, contacto no covalente omitido, source compartido, remove/undo y stale pick.

### 5. Representaciones y overlays completos

- **Cartoon/backbone/trace/tube/putty/nucleic/bases/base-pairs:** aprovechar perfiles/splines actuales; publicar shoulder/length/tip de arrow, radial sides, tolerance/max steps y nucleic block/ring/thickness/outline. Tip terminal real, no fin artificial de selección; gaps por missing residue/coords/link, no cambio de chain o selección excluida. Dash connector y direction wedge con provenance. Purine 5+6/pyrimidine6 reales; glyph incompleto se omite con semántica definida. Recetas source-row para que slabs/rings sigan trayectoria, no static frame inicial.
- **Atoms/sticks/lines:** conservar spacefill/ball-and-stick; single/double/triple/**quadruple**/aromatic/zero/metal fieles, orden distinto de strands, frame de offsets estable. Control Split/Interpolate de color y caps Exposed/All/None; junction compartida sin seam ni bias arbitrario. Lines/crosses con coverage analítica y ancho físico, mismo hit/depth/rank que pick, alpha consumida realmente por AA/OIT.
- **Points vs Dots:** Points sigue marcador; Dots produce sólo superficie vdW/SAS expuesta con density/probe/include_parent explícitos y sampler MolFrame. Target y universo de occlusion separados; positions/pick fuente y cache por coords/policy, scratch reutilizado durante trayectoria.
- **Superficies vdW/SAS/SES/Gaussian/blob/mesh/dots:** mantener algoritmos existentes y sumar carve/clear por selección/instancia/cutoff/normal gate; carve conserva si algún target cumple, clear elimina después. Empty carve oculta/empty clear no elimina; máscaras iguales en beauty/shadow/pick/export. GaussianVolume consume rho unit-weight con sigma/support/grid explícitos y transfer; comparte field resident con Gaussian surface, sin GPU→CPU→GPU ni presentarlo como densidad electrónica.
- **Glycan/SNFG:** ribbon existente y opción símbolos completos/clasificación estándar/links/rings desde MolFrame; catálogo con formas/colores de referencia y glyph unknown explícito. No llamar SNFG al twister coloreado. Símbolos y enlaces siguen pose y pick fuente.
- **Orientation/polyhedron:** axes/box/ellipsoid con principal axes/extent, signos y degeneracy deterministas; no PointGlyph falsamente orientado. Coordination hull existente con faces coplanares agrupadas/trianguladas una vez (sin triples solapados); >budget error, shells sin volumen no inventan polyhedron. Fuente bonded vs cutoff explícita.
- **Labels:** literal/atom/residue/property con precisión/missing declarados, fonts/style/size/background/connector/outline/offset/priority reales. Quitar uppercase e ignored color. FontBinding atlas SDF+glyph/kerning validado/content hash, U+FFFD para glyph faltante; assets reales y licencia, sin descubrir fonts OS por frame. Formatear sólo al cambiar datos; anchor GPU sigue coords.
- **Measures:** styles de línea/dash/arc/label y sagitta tolerance, no veinte segmentos fijos. Dash por longitud acumulada; torsión signed/degeneracy coherente. Un resolver de anchors/value para label/render/pick; value/unidades actualizados en trayectoria/placement, no fórmula separada.
- **Guías/ADP/beads/coarse:** verificar y publicar caminos ya existentes; plano/celda affine y map extent de esquinas reales, no AABB que pierda skew. ADP usa tensores upstream actuales; no reparsar ANISOU. Alignment object dibuja correspondencias de coords colocadas con FitResult existente; fitting sigue upstream/caller.

Smoke: misma escena y picks por Rust/Python/WASM/commands; zoom/cuts, DNA/RNA incompleto, sheet terminal, glycan ramificado, hull cubo, lowercase/Å/°, label roja, arco180°/torsión negativa, edición/inverse y trayectoria viva. Golden sólo después de fijar fixture/cámara.

### 6. Density y segmentación públicas

Cutover VolumeSpec/Binding origin+spacing a un único voxel_to_world column-major, dimensions y Arc values. ScalarVolume ya lo soporta. Derivar affine MRC upstream con starts/sampling/cell/origin/axis-order canonizado y comprobar contra sample_cartesian; f64→f32 validado. Source hash/matches incluye matriz completa.

VolumePresentationSpec reutiliza Direct/Isosurface/Medium/LiquidSurface/Slice y transfer/opacity/step/region existentes; exponer Mesh/Dots de isosurface con extracción compartida, vertices/edges dedup y normales world inverse-transpose. Isovalue/transfer no regeneran rho Gaussian innecesariamente. Slice oblique usa world→voxel/trilinear; categorical nearest-label, nunca interpolar IDs. Isovalue sin crossing produce geometry vacía válida; binding faltante queda unresolved observable, no screenshot vacío aprobado. Admitir escena sólo volumen sin exigir molecular source ficticio.

SegmentationSpec/Binding/ID/domain/patch/commands nuevos bajan a SegmentedVolume/SegmentStyleTable existentes. Labels u32 sparse incluidos0, styles únicos; label sin style transparente, empty styles cero drawables. Style edit no reupload labels; undo restaura el estado. Lowered map **SegmentationHandle→SegmentationId** y captura source_id→handle+generation: pick nunca el primer scalar volume. Dos segmentaciones con mismo label y handle reciclado prueban la identidad.

Map extent incluye grid/crop y skew; alignment connectors preservan ambos endpoints. Tests de MRC skew/permutation/origin, standalone direct/slice/mesh/dots, transfer/iso±, crop, labels grandes0, bind mismatch, remove/undo y pick exacto.

### 7. Colour, size y appearance declarativos

- Completar source categories polymer/SS y placement categories unit/operator/assembly/model con dominios separados; no almacenar operador por atom source ni pasar IDs u32 por f32 truncado. Mantener CPK/residue/chain/entity/molecule/sequence/carbon rules y métricas occupancy/B-factor/charge/hydropathy/SASA ya presentes.
- Catálogo único scene para names/ramps/palettes; command/bindings lo consumen. Comparar conjuntos y valores reales Mol*/PyMOL, incluir reversals/CVD. Colisiones: names actuales conservan valor; namespace molstar:/pymol: resuelve referencia exacta. ScalarRamp shared con todos los anchors (bound explícito4096), LUT generado una vez; custom ramp usa mismo kernel/legend, no registro global mutable ni16-stop truncation.
- Fields parametrizados VolumeValue/relative, distance-to-isosurface world y electrostatic potential con charge property/provenance/units/ramp/domain/missing. Coulomb screened/softened se nombra como tal, no APBS/Poisson–Boltzmann. PEOE/chemistry upstream; exacta suma referencia y aceleración sólo con cota de error medida. Volume sampling affine, distancia world BVH, sigma0/exterior/iso vacío missing explícito; no EDT de índices skew.
- Size Physical/Uniform/Uncertainty/VolumeValue con parámetros y fórmula de referencia declarados; radio negativo/NaN error, exterior missing. Baja al registry RadiusScale/WidthScale y bounds conservadores; implementar consumo ribbon antes de anunciarlo.
- AppearanceChannels por target: color/opacity/emission/substance/wiggle/reset, priority explícita; orden base → rules(priority,ID) por canal → VisualStyle explícito → marker compositor. Property/metric/field schemes con keys completos, no colapsar Other. Extender VM/VisualStyle existentes para emission/roughness/specular/material-strength/radius/width/position-offset, sin segundo shader por rule. Wiggle determinista GPU con tiempo único y displacement bound; no mutar coords moleculares ni medidas químicas por accidente.

Prueba: prioridades/reset y rules solapadas, dos estructuras/instancias, missing fields, bounds/torsión/poses, transparencia re-clasifica routing, emisión HDR real y catálogos completos por valores. Unsupported family/output falla antes del draw.

### 8. Picking, hover y marker ghost

Ampliar el **PickReadback detached existente**, no wrapper async &mut self. Cada request posee bytes/captura inmutable de páginas/tickets, placements, segmentos y frame key (renderer/scene/content/coords/cámara/proyección/extent/visibility/clip/displacement). Resolver el sample capturado, nunca Picker.submission actual. Pool bounded/fence-safe, drop/dispose libera; backpressure explícito, no cola ilimitada. Página/handle reciclado y cambios incompatibles dan stale tipado; bytes truncados no son background.

WASM beginPick/resolve detached/finishPick transporta sample opaco con provenance; eliminar método async mutante que mantiene RefCell borrow y migrar callers. Viewer permite draw durante await, comprueba scene+view epochs/latest token y descarta completions tras dispose. Regresión dos picks intercalados más renderCamera/resize/scene edit; no panics ni owners equivocados.

Hover low-resolution cache 1/4 de ancho/alto físico, un refresh como máximo por frame/key. Reducir tuple completo (depth/source/page/row/instance/segment-label) por reversed-Z nearest/tie estable; movimiento sobre escena estática lookup CPU sin readback nuevo. Click exacto full-resolution no usa aproximación. Cache stale inicia refresh/coalescer, nunca respuesta vieja válida. Marker-only no invalida pick geometry; Hidden/opacidad participativa/pose/radio/clip sí.

SelectionLoci tipado por source/instance y rangos normalizados, expansión usando tablas MolFrame: atom/residue/chain/entity/model/operator/structure/instance y atom/residue/chain-instances, no strings resid+chain. Queries conservan parser; viewer replace/Shift-add/Alt-remove/Escape-clear usa loci exactos y patches con inverse. Missing metadata/granularidad no disponible error; overlays no fingen atom rows. Mantener focus/context/hidden/muted y custom channels.

MarkerStyle + máscaras visible/occluded de full extent para selected/focused/hovered, blend Max y prioridad única; intersección/depth/clip compartidos por familia incluidos transparentes. Único compositor en el orden transversal; retirar edge deferred/analytic rims obsoletos. Ghost no cambia entity/depth/pick y no acumula trails. Smoke real hoverA→B→clear/click/undo en proteína, sphere occluded, capsule/cartoon/surface/instancia/segment con fuerza0/.3/1 y resize/dispose; counters demuestran que pointer motion quiet no submit/readback.

### 9. Render: calidad completa y optimización medida

Baseline/per-pass primero. Optimizar según coste observado, nunca según cifras históricas: CPU sync/maps/collect por revisión con scratch; licorice bounds/early rejects exactos, payload near-plane y caps; AO/shadow scene-wide y geometry dedup; dirty surface generation/erosión/normales; conservative minmax-brick/DDA y root refinement sin subir step floor; uploads edit-scoped. Mismo output/settings antes/después, timings/counters más silhouette/depth/source y calidad. No claim de speedup sin medición.

Para spacing0,25Å efectivo en grandes campos, quitar cap192/coarsen: bricked logical grid/atlas con halos suficientes (interpolation y SES probe), reutilizando residency/budgets existentes. Minmax sólo omite bricks sin crossing demostrado, sin asumir SDF Lipschitz. Full detail que no cabe da presupuesto/incomplete explícito; nunca Highest con proxy/coarsen certificado complete.

Completar efectos:

- Multi-scale SSAO con radios Å, near/far thresholds, output full-res y depth auxiliares conservadores; matriz1/2/4 escalas ×16/32/64 taps. Traced AO distinto de SSAO. AO history/reprojection depth/normal/entity y sample-count/moments; camera/disocclusion/pose/clip/light/opacidad invalida, marker edge-only no. Validar TAA/jitter/DOF/bloom/motion/shadows/fog/outline existentes con renders de dispositivo y quiet/moving frames.
- AntiAliasing enum None/Fxaa/Smaa con options validadas; eliminar boolean edge_smoothing y migrar todos callers. SMAA edge/weights/neighborhood, tables reales/licencia/checksum y resources append-only. Una sola elección, no FXAA+SMAA ni fallback tácito. CAS full-res después de AA, sharpness/denoise declarados, alpha preservado.
- Transparency WBOIT existente y DPOIT reversed-Z/front/back pingpong/source-over; HAL blend/format capability explícita. Productores opacos/translucent/volumes usan mismo material/clip. Budget layers y residual observable; maximum/export falla por contribuciones no peeled, no tail silencioso. Probar3/17 layers orden inverso/occluders y alpha conocida.
- Material existente ampliado con x-ray edge/inverted, bump estable y interior color/opacity de cap/backface. Un campo authoritative opacity, mismo shader material para todas familias/GI; bump no cambia silhouette/pick. Cel existente se ejercita.
- ClipSet existente generalizado plane/sphere/box/cylinder/cone con affine/invert; AND de retained predicates y ray multi-interval fijo/bounded, normals inverse-transpose. Caps/material/mesh/surface/volume/shadow/GI/pick coherentes, no cap plano falso en curved cut. Validar inside/tangent/parallel y shapes compound.
- Background gradient existente más image/skybox assets generacionales renderer-owned con fit/rotation/alpha; caller decode/fetch, nunca red en engine. Face order/seams/transparent export y stale assets explícitos.
- GI path tracing portable full-scene: TLAS/BLAS compartidos, analytic atoms/bonds/primitives, ribbon/mesh, implicit fields, affine placements y media density; closest/any-hit con material/clip/source, no atom/bond AO anunciado como GI. Diffuse/GGX, direct/environment sampling, throughput/pdf y compensated roulette; deterministic seed/convergence/invalidación. Unsupported geometry error, no occluder invisible. Hardware path sólo si feature negociada/enabled y parity probada; ninguna arquitectura backend paralela.
- Hi-Z reversed-Z **min** conservador, odd sizes/background0/near-plane invalidation; comparar on/off source/depth. Stride-prefix/Bayer LOD es opt-in aproximación declarada; Highest stride1/no thinning. UploadRing budget changes conservan live tickets/fences, capacidad reservada y shrink sólo bloquea admissions; no ring rebuild inseguro. Highest espera full residency.
- Gate warm steady: cero heap allocations/reallocations internas evitables y cero creates GPU evitables; reservar antes del loop, arrays/arena/scratch y counters de alcance. Reportar driver/backend allocations aparte, no proclamar global0 por residency delta. Cold path reserva y se mide.

Stereo: off-axis parallel eye cameras, projections externas validadas, history/AO por ojo y layouts de referencia (cross/wall/side-by-side/interleaved/checkerboard/anaglyph/separate/alternating/clone/custom). Quadbuffer/geowall exige compositor/capability de host, no promesa del swapchain. ExternalView/render-to-borrowed-target y escala Å→m, sin readback puente para XR. Zero separation/disparity/projection rays y64completed por ojo.

XR inmersivo permanece en alcance, con prerequisite explícito: la integración wgpu actual no transmite xrCompatible al pedir adapter; WebXR/WebGPU exige ese adapter antes de crear device. Buscar release/upstream seguro disponible, negociar feature webgpu y mismo device/import seguro; no unsafe/monkeypatch/vendor/backend alterno. Hasta disponer soporte y browser/headset/session reales, terminar external views/stereo y reportar el bloqueo concreto; no marcar XR completo ni simular éxito. Permisos/session pertenecen al host y requieren aprobación interactiva al ejercitarlos.

### 10. Orientación, snapshots, presets y exportación

Orient-to-axes stream world coords/covarianza f64 y eigensolver MolFrame con basis determinista para degeneracy; no copiar columna. CameraPath/Keyframe/Easing/Bookmark y Timeline/TimeWarp/PlaybackMode actuales se publican en namespace curado, deserialization validada y un reloj global. No otro interpolador/editor.

Snapshot único interop::SceneSnapshot; retirar DomainSceneSnapshot y MovieExportRequest duplicados una vez migrados callers. SceneSpec ya posee cámara/assembly/styles/interactions; añadir tiempo authored canónico y hacerlo entrada del timeline existente, no un segundo reloj. Capture hash del estado completo y refs/content identities, sin pixels/coords/buffers. Restore prepara bindings/Resolution, valida todo y swaps atómicamente; revision avanza, no vuelve atrás. RestoreSnapshot inverse captura estado previo; borrar almacenamiento recursivo extensions snapshot. Named capture/restore/remove en SessionSpec, no app editor. Smoke snapshot→mutaciones→restore recupera imagen/pick/camera/tiempo; fuente/hash faltante no produce restore parcial.

Presets catálogo único de once: Empty, Auto, AtomicDetail, PolymerCartoon, PolymerAndLigand, ProteinAndNucleic, CoarseSurface, Illustrative, MolecularSurface, AutoLod, Mesoscale. Reusar builder auto/clasificación y opciones reales hydrogen/carbon/symmetry/SNFG/unlit/graphics/LOD. Cada receta con clases/formas/materials de referencia, no sólo registrar names. Atomic replacement del target preserva otras estructuras/overlays; inverse restaura IDs. Performance/Balanced aproximados opt-in; Highest no acepta auto-dropping como máximo detalle.

Image: PNG existente, JPEG matte/quality y WebP lossless/lossy reales, write/encode únicos y extensión correcta; alpha compositado linear sólo cuando formato lo exige, sin segunda transferencia sRGB. Verificar licencia/API/feature wasm de encoders antes de fijar dependencia; no librería enorme/FFI sólo para dos codecs. HDR/OpenEXR existente se conserva/expone, no duplicar. Crop físico/transparent background/axes y profile/camera/effects del engine, no screenshot DOM. Begin-image detached conserva target/layout/fence y permite render durante await.

Geometry export: un gather a muestra/placement/material efectivo y writers reales GLB/glTF(+bin), OBJ(+MTL), STL, USDZ, DAE, WRL, IDTF, POV. Source coordinate units/calibración explícitas, matrices/normales/provenance y color/opacity; teselación sólo export de impostores con chord-error, ribbons actuales y misma isosurface/SES/carve field. CPU output allocation necesaria es consumer-owned, no licencia para allocation por atom en draw loop. Labels/screen overlays/direct volumes no mesh generan diagnóstico de omisión explícito, no cube sustituto; geometry soportada que falla aborta export. USDZ stored/alignment64, índices/buffers/sidecars válidos; independientes loaders verifican contenido/units/bounds/material y placements. Sin inventar U3D encoder ni requerir collada2gltf externo.

Secuencias: SequenceExposure Converged usa el exposure común completo por nueva cámara/tiempo, bounded in-flight/order/cancel/device errors; Progressive explícito preserva sólo historia compatible. Declarative paths/keyframes/snapshots se muestrean determinísticamente. Tres frames distintos deben tener presupuesto completed correcto y no ghost temporal; JPEG/WebP/PNG codificados de cada frame y hash/orden/time reportado. Codificación MP4/GIF/editor movie/docking/GUI quedan explícitamente en consumidor por la decisión de alcance, no «features implementadas» del engine.

### 11. Integración por unidades y cierre del ledger

Integrar en orden: medición/baseline → química/SS/policies → placements/provenance → formas/volumes → colour/appearance → pick/ghost → render effects/performance → presets/snapshot/export/stereo/XR → matriz end-to-end completa. Pueden ejecutarse slices verdaderamente independientes en paralelo después de fijar contratos compartidos; no dos agentes editando ownership/graph/IDs a ciegas.

Cada unidad incorpora cambios de fachada, bindings, command registry/planner/parser/printer, patch/inverse, JSON/stubs/ejemplos y documentación de contrato. Tras smoke real, actualizar mapping.md/missing.md con owner/evidencia/estado y blocker exacto; mantener [~] donde falta comparación. No doc final que esconda parcial ni marcar una feature por compilation. No tests de wiring/wording/default literals/source text; retirar los obsoletos, no repin. Regresiones permanentes sólo para bugs consumer-visible: stale provenance, affine identity, owner categorical, restore atómico, sequence convergence, AO/marker history, clip interval, transparency overflow, conservative culling y fence ownership.

## Archivos críticos

Rutas existentes; nuevos módulos se sitúan junto al owner, no en lib/mod:

- MolFrame: `molframe-chem/src/{bonds,secondary,standard_bonds}.rs`, source enums/core tables, CIF/PDB lowering/writers, `molframe-query/src/language/ast.rs` y consumers; `molframe-xtal/src/{assembly_spatial,mrc}.rs`; surface sampler/geom/PEOE y fachada.
- MolGFX source/placement: `molgfx-core/src/structure/source.rs`, dataset/structure/gpu tables y scene/inspect; `molgfx-scene/src/{spec,scene,patch,representation,overlay,appearance,visual,property,interop,preset}/`. Prioridad `scene/runtime.rs`, `scene/overlay_pick.rs`, `render/pick.rs`, `interop/snapshot.rs`.
- Renderer: `molgfx-render/src/engine/{image,picking,profiling,graph_setup}.rs`, scene_gpu/sync/shared_caches/draws/segmentations, passes/fields/BVH/label packing; HAL descriptors/queue/readback y molgfx-wgpu conversión/capabilities; WGSL único de molgfx-shaders.
- Publicación: fachada `molgfx`, command ir/session/registry, PyO3 bindings/.pyi y WASM contract/session/camera. El host canónico del canvas pertenece a `web/`, fuera del engine Rust y del crate WASM; React y AnyWidget adaptan ese mismo viewer. Consola, Workbench y UI de aplicación pertenecen a MolStation.
- Medición: molgfx-bench fixtures/metrics/runner, `bin/{frame_time,resources}.rs`, Cargo registro binary y manifest de comparación; suites browser/Python existentes y ledgers.

## Verificación y criterios de aceptación

### Calidad y performance

- 1280×720 y1920×1080 **físicos** para aceptación;4K adicional. Metal/WebGPU separados; registrar CSS/DPR/render-target/capabilities enabled, OS/toolchain/browser/revisions/dirty y políticas efectivas.
- High fijo: superficie spacing **efectivo**0,25Å, ribbon8 y64 muestras convergidas por output, toda residency/geometry, stride1, no adaptive resolution/count tier/truncation. Presupuesto para iluminación/transparencia/AA visible. Máxima calidad fija de cada algoritmo; receta GI adicional64samples/4bounces, AO multi-scale completo y escenas transparentes DPOIT sin residual. Efectos artísticos como fog/DOF/bloom no se activan arbitrariamente en todos los casos; se fijan en manifest y se prueban también juntos.
- Por caso/proceso:120 outputs warmup +1200 medidos, tres procesos aislados. Cámara/trajectory/marker/opacity/clip/residency además deestática. p50/p95/p99 de completed latency, input→completed, CPU record/submit, GPU resolved sólo; primer frame/cold, uploads/edit, in-flight/backlog, heap allocations, logical GPU bytes y RSS real. Sin timestamps, GPU percentiles null con resolved_count, no falsos0.
- Cumplimiento120: >=120 outputs completos/s y p95 frame/input→completed<=8,333333ms con cola acotada; p99 siempre reportado. No contar submuestras, cached no-ops, rAF callbacks ni idle settled. Throughput batch y latency serial separados. Si falla, meta **incumplida**; no reducir calidad ni workload para declararla aprobada.
- RSS peak real por proceso: medidor OS (Darwin time -l con unidades preservadas) sobre child binary, no cargo compilation; browser renderer/GPU/tree identificado. Logical GPU residency no es VRAM física del driver. Instrumentar StatsAlloc en harness con output/scratch reservado; reportar scope engine/backend/driver y consumer-owned pixels/export por separado.
- Outputs run.json/settings/hashes, frames.jsonl, passes.jsonl, summary.json/csv, RSS raw, screenshots y diffs/visual checks; no raw RGB equality entre defaults de engines. Semántica/counts y geometría comparables; tolerancias físicas/ROIs fijadas antes de after-output. MolGFX same-adapter/seed mantiene determinismo donde se promete.
- Harness actual: `nix develop -c cargo run --release -p molgfx-bench --bin parity -- --manifest crates/molgfx-bench/parity/corpus.json --cache /ruta/al/corpus --output target/parity/highest-720 --size 1280x720 --warmup 120 --outputs 1200`. La ruta del corpus debe existir y contener los hashes requeridos. Repetir para 1920x1080 y 3840x2160 y tres procesos aislados; seleccionar recetas con `--recipe` repetible según el manifiesto. Preparar los ejecutables externos y comprobar las recetas efectivas antes de medir. Es un comando de aceptación pendiente, no una ejecución reportada.
- Cold WASM release: context/cache nuevos, import/download/compile+instantiate/adapter+pipeline/parse+scene/upload/first **completed** output, module/download/JS heap/RSS separados;40 trials,100k/500k/1M más fixtureprotein. Reusar build/runtime/server, no segundo runtime. Conservar browser errors y screenshots de UI real y interaction smoke.

### Suites y smoke obligatorios

Desde cada repo, usando Nix y entorno de bindings correcto:

**MolGFX**

- `nix develop -c cargo fmt --all --check`
- `nix develop -c cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `nix develop -c cargo test --workspace --all-features`
- `nix develop -c cargo check -p molgfx-wasm --target wasm32-unknown-unknown`
- `nix develop -c cargo clippy -p molgfx-wasm --target wasm32-unknown-unknown --all-targets -- -D warnings`
- Rebuild extensión Python con maturin/entorno Nix del repo; `python -m unittest discover -s python/tests` y `python -m mypy.stubtest molgfx._engine` contra extensión nueva, no wheel viejo.
- `nix develop -c wasm-pack build crates/molgfx-wasm --target web --release --out-dir pkg`; el paquete debe traer `molgfx_wasm.d.ts`. Skips por falta de device no certifican aceptación.
- Policy scans según RULES usando herramientas especializadas: cero allow/expect lint attrs, unwrap fuera de tests, unsafe no autorizado, source >500 líneas Rust/WGSL; independencia de bindings/facade y mod/lib sólo declaraciones.

**MolFrame**

- `nix develop -c cargo fmt --all --check`
- `nix develop -c cargo clippy --workspace --all-targets -- -D warnings`
- `nix develop -c cargo test --workspace` y `nix develop -c cargo test -p molframe --doc --features full`
- `cargo check -p molframe` y check --no-default-features, después **cada** feature individual: pdb, mmcif, bcif, modelcif, geometry, ic, query, spatial, chemistry, interop, crystal, surface, analysis, validation, sequence, compare, trajectory, audit, motif, adapters, gzip, zstd, mmap, dentro de Nix. Actualizar matriz si manifest real cambió.
- Policy cap500 Rust/unwrap/expect y boundaries; benchmark upstream del camino añadido más consumer smoke Rust/Python.

Tests no son toda la prueba. Por unidad, lanzar consumidor real/throwaway script de la fachada, alterar escena, observar pixels/pick/value/completion y errores; retirar scaffolds después. Browser: usar browser.open, acciones reales y screenshot fresca; close al terminar. Comprobar snapshots, commands/undo, picking concurrente y exports en consumidores independientes. Repetir verificación final **sin** CARGO_HOME override temporal, con pins Git publicados.

## Supuestos, riesgos y bloqueos explícitos

- La aprobación autoriza implementar este alcance, no convierte historial en resultados actuales ni garantiza que120FPS máxima calidad sea físicamente alcanzable. Reportar metas incumplidas con medidas y costes dominantes; no solución adaptativa sustitutiva.
- Hardware actual30Hz impide certificar120 presented frames/s. Display>=120Hz y evidencia de presentación son requisito de ese claim; offscreen completion medido es distinto.
- XR browser requiere adapter compatible soportado por dependencia upstream segura y headset/browser/session. No cerrar gap hasta smoke real; finalizar todos los caminos alcanzables y nombrar dependencia/hardware faltantes si sigue bloqueado.
- Fixtures ausentes deben incorporarse con bytes/licencia/hashes reales antes de correr comparación. No usar inventario/counters históricos como baseline ni mock para Mol*/PyMOL no disponibles.
- No enmendar trabajos del usuario fuera de esta paridad ni hacer limpieza ajena. Docs/contracts afectados sí se actualizan en su unidad, después del smoke. Sin release/publish ni reescritura de Git.

## Estado de ejecución al consolidar el plan

- Guías/near-plane: smokes nativos y browser e inverse/validación observados; conservar esos límites de evidencia.
- Medición: HighestFixed, exposición/fences, metadata Rust/Python/WASM y harness implementados parcialmente. Pendientes residencia completa real, cierre del profiler y baseline máxima completa.
- Gemmi: primera unidad recíproca implementada y contrastada (530 Hall, 386.370 reflexiones, 2.187 spacings). El resto de paridad sigue abierto; no sustituirlo por esta unidad.
- Las fases 3–11 conservan todos sus criterios. Inventario existente, compilación o smoke reducido no equivalen a aceptación completa.
- Prioridad inmediata: corregir full_residency que hoy sólo comprueba tickets de upload; confirmar lifecycle/stale del profiler; completar corpus antes de optimizar.
- Publicación, pins definitivos, suites completas y objetivos de rendimiento no se consideran cerrados por este documento.

### Desglose adicional de Gemmi (owner: MolFrame)

1. Conservar y verificar formatos estructurales CIF/PDB/BCIF/mmJSON, modelos y monómeros/restraints existentes; comparar semántica y roundtrip sin crear un segundo modelo.
2. Verificar catálogo de simetría, operadores exactos, celdas, vecinos crystal/assembly y geometría recíproca ya presentes.
3. Unificar geometría affine de mapas MRC/CCP4, sampling/trilinear, starts/origin/axis-order/skew y rutas block/brick; contrastar valores y coordenadas, no sólo dimensiones.
4. Auditar e implementar gaps seleccionados de scattering y factores de estructura, con unidades, factores atómicos, ocupación/ADP, simetría y referencias numéricas explícitas.
5. Auditar e implementar procesamiento de reflexiones y transformadas mapa↔coeficientes/FFT donde falte: convenciones de fase, normalización, índices, simetría y datos ausentes deben quedar explícitos antes de elegir implementación.
6. Cada capacidad termina con facade Rust/Python, errores y casos límite, diferencial real con Gemmi, benchmark y documentación. No afirmar equivalencia total por un subconjunto ni añadir dependencia Gemmi al producto.
