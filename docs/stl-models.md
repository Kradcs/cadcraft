# STL 3D workspace

The experimental Windows portable v5 build opens standalone triangle meshes in
binary or ASCII STL. Use **File → Open STL Model…** (**Arquivo → Abrir modelo STL…**),
the normal Open dialog, or drag an `.stl` file onto CADCraft. Each mesh opens in its
own document tab; existing 2D drawings remain in their tabs.

## View and adjust

- Left-drag orbits the camera; right/middle-drag pans; the wheel zooms.
- **Fit model**, Front, Back, Right, Top and Isometric choose useful camera views.
- The workspace reports the full triangle count and the transformed X/Y/Z sizes.
- Choose an axis, then rotate or resize proportionally. Z is height.
- Scale changes all axes uniformly around the model center. Move applies X/Y/Z
  offsets. Center on ground centers X/Y on the origin and sets the lowest Z to zero.
- Geometry changes use the document's normal Undo/Redo and unsaved-change prompt.
- **Save STL As…** exports all transformed triangles, with recomputed normals.
  Ordinary Save writes the current STL. Saving the mesh as a 2D format is refused
  to prevent losing its geometry. Mesh autosaves use STL content for recovery.

STL does not specify units. **Original STL unit** labels the source coordinates
without resizing them. Select the original unit explicitly before requests in
centimeters or millimeters. CADCraft preserves its chosen label in its own STL
header; other programs may ignore this nonstandard metadata and need an explicit
unit choice on import. Geometry is always stored as ordinary STL triangles.

## Codex

Open **Window → Codex Assistant**, connect, and start a new conversation when
switching to a different document. Examples after identifying the original unit:

> Defina a unidade original como milímetros. Deixe o modelo com 100 mm de altura,
> mantendo as proporções, centralize no chão e enquadre a vista.

> Gire o modelo 90 graus no eixo Z. Confira as dimensões finais.

The agent can inspect the mesh, discover parameters, move, rotate, scale, resize,
center, set/convert units and select camera views. Geometry commands are
`mesh.inspect`, `mesh.move`, `mesh.rotate`, `mesh.scale`, `mesh.resize`,
`mesh.center`, `mesh.units`; camera presets use `ui.mesh.view`. They execute the
same operations as the interface. File selection and saving remain under user
control. Commands for 2D geometry are disabled in mesh documents, and document
identity checks reject tool calls intended for another tab.

The model provider receives mesh statistics and tool results, not the complete
STL or a rendered image. These tools support explicit dimensions and transforms;
the agent cannot infer the character's appearance from mesh statistics alone.
Existing PDF/image/TXT reference attachments remain available.

## Scope and limits

This is a whole-mesh viewer and transform editor. It does not provide character
generation, vertex sculpting, slicing, Boolean operations, mesh repair, texture or
material editing, parametric solids, or OBJ/STEP/IGES import. Facet attributes and
nonstandard STL colors are not reproduced; geometric normals are regenerated.
Export does not certify watertightness or printability; check those in a slicer.

Limits: 64 MiB per file, 1,000,000 triangles, finite coordinates within ±1e9 units,
and a minimum overall size of 1e-8 units. The view renders all triangles using a
software depth buffer on a worker thread on desktop, at up to 1280×1024 pixels.
A bounded rasterization budget reports an error for an excessively expensive
view; **Fit model** or zooming out can resolve it. No mesh simplification is used.
The wasm build retains a synchronous bounded renderer.

Synthetic tests cover parser damage/truncation, binary/ASCII round trips,
geometry/unit conversions, Undo/Redo, blocked operations, document switching,
depth ordering, UI controls and agent tool calls. Additional local validation used
the user-supplied 131,256-triangle STL to resize, rotate, center, export, reopen
and render. The supplied file and its previews are not repository or release
assets. A signed-in model request remains a separate validation on the user's PC.

The full `cargo xtask ci` gate currently stops on pre-existing Clippy warnings in
`geom/src/nurbs.rs` and `xtask/src/layers.rs` with Rust 1.95. Targeted feature and
file-lifecycle tests, desktop/wasm compilation, formatting, assets and dependency
layer checks are run separately. The Windows download workflow requires native
feature tests and both STL and 2D file smoke tests before publication.

## v6 measured construction

Imported STL remains a whole-mesh workflow. New editable CAD3D projects additionally support named primitives, extrusions, revolutions, faceted boolean operations and STEP export; see [3D modeling](modeling-3d.md). These are not analytic CAD solids.
