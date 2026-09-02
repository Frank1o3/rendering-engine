# Rendering Engine Improvement Plan

## Overview

This project already has a solid base for a lightweight OpenGL renderer: a persisting mapped transform buffer, a shared geometry pool, per-material pipeline tracking, a dirty-scene optimization, and indirect draw batching support. The current engine architecture is promising, but there are several practical improvements that would materially increase performance, improve maintainability, and expand functionality.

This plan focuses on the engine code under `engine/src/`, not the demo application.

---

## Current strengths

The engine already contains several strong ideas:

- Persistent CPU/GPU mapped buffers reduce per-frame allocation churn.
- `Scene` keeps a cached order and only refreshes dirty transforms instead of re-sorting every frame.
- `GeometryPool` consolidates mesh uploads into shared GPU storage.
- `Renderer` batches draw calls by `(material, mesh)` to reduce GL state changes.
- `PipelineCache` avoids repeated pipeline-state creation.
- `TripleBuffer` provides a clean render-thread data handoff pattern.
- `IndirectBuffer` lays a good foundation for future draw submission improvements.

Those are the right ideas to build on. The next phase should focus on eliminating hidden costs and expanding the engine beyond a single-pass, single-primitive renderer.

---

## High-priority performance improvements

### 1. Replace heavy per-frame vector churn with non-allocating frame structures

Status: [x] Implemented

Current observations:

- `render()` recreates or grows `sorted_instances` and temporary vectors each frame.
- `visible_indices` is allocated every frame during instance culling.
- `pending_draws` is cleared and rebuilt each frame, even when the scene is mostly static.

Recommended changes:

- Add recycled frame scratch buffers to the renderer, reused across frames.
- Pre-allocate `sorted_instances`, `visible_indices`, and `pending_draws` to a conservative capacity.
- Separate static and dynamic render lists so static objects do not re-enter the expensive per-frame submission path unless they changed.
- Consider a lightweight command buffer abstraction that owns reusable scratch memory for the render thread.

Why it matters:

- This reduces allocator pressure and GC-like behavior in Rust, especially under high object counts.
- It improves frame stability and makes performance more predictable.

### 2. Improve culling and visibility filtering

Status: [x] Implemented (coarse spatial visibility window)

Current observations:

- Frustum culling is implemented in `render_instances()` using per-object sphere checks.
- This is a good start, but it is still doing full scene traversal every frame.
- Dynamic objects and UI commands are mixed into the same sorted-instance pass, which increases complexity.

Recommended changes:

- Add a spatial partitioning step using a coarse grid, BVH, or chunk-based scene partition.
- Keep a per-frame visible list rather than scanning all objects every time.
- Add optional occlusion culling and LOD selection for large scenes.
- Split 3D world draws from UI draws into separate render lists and separate pass logic.

Why it matters:

- Large scenes will become dominated by culling and list maintenance cost unless visibility is reduced earlier.

### 3. Add proper render pass/resource layering

Status: [x] Partially implemented via pass helper methods

Current observations:

- The renderer is effectively a single monolithic pass with skybox, world, and UI logic embedded in `render()`.
- State switching is managed manually and repeated per pipeline group.

Recommended changes:

- Introduce a small render-graph or pass abstraction.
- Define passes such as `Opaque`, `Transparent`, `Shadow`, `UI`, and `PostProcess`.
- Let each pass own its render state, clear behavior, viewport, and resource bindings.

Why it matters:

- Makes pipeline state transitions cleaner and easier to optimize.
- Enables future features like shadow maps and deferred rendering without reworking the core renderer.

### 4. Optimize pipeline and state binding logic

Current observations:

- `apply_pipeline()` checks pipeline equality each time and binds program, cull mode, depth state, and blend state.
- Shader uniform updates are done ad hoc, and there is no broader cache for uniform blocks or binding slots.

Recommended changes:

- Cache bound pipeline state explicitly on the renderer and avoid redundant state changes.
- Add a uniform buffer object (UBO) or material block system for common per-frame constants.
- Group dynamic per-draw uniforms into a shared block instead of querying shader locations repeatedly.
- Use separate shader reflection or metadata for known uniforms rather than repeated string lookups.

Why it matters:

- State changes are a major CPU-side cost in OpenGL rendering code.
- It also makes the engine more scalable as more material types are added.

### 5. Improve the transform buffer model and synchronization strategy

Current observations:

- The engine uses `PersistentMappedBuffer` plus `region_fences` to avoid re-uploading instance transforms every frame.
- This is a good pattern, but region reuse is still fairly simple and all object transforms are packed into one buffer.

Recommended changes:

- Add a per-frame instance write cursor with explicit dirty pages.
- Support multiple transform buffers and per-instance ranges for large worlds.
- Tighten synchronization around write regions so the renderer does not wait unnecessarily.
- Consider alternate strategies such as separate static and dynamic transform streams.

Why it matters:

- This is one of the most important bottlenecks for large scenes with lots of moving objects.

### 6. Expand MDI strategy support to a real backend-aware path

Current observations:

- The `IndirectBuffer` supports a strategy enum and comments explicitly note that `glMultiDrawElementsIndirect` is not exposed by `glow` in current usage.
- The code is already structured for a future real MDI path, which is good.

Recommended changes:

- Add backend detection at startup and choose the most efficient call path for the active GL version.
- Preserve the emulation path as a stable fallback, but prefer the native path when available.
- Add a benchmark or debug option to compare single-draw vs. MDI behavior.

Why it matters:

- Real MDI can significantly reduce driver overhead for large draw counts.

---

## Resource-management improvements

### 7. Strengthen resource lifetime management and hot-reload support

Current observations:

- `ShaderManager`, `MaterialManager`, `TextureManager`, and mesh/geometry management all use IDs in a straightforward way.
- Resource lifetime is controlled by a map keyed by IDs, but there is no explicit lifecycle policy beyond creation/drop behavior.

Recommended changes:

- Add explicit resource lifetime tracking for shader hot reload and texture re-upload.
- Add `ResourceHandle` wrappers with generation counters to prevent stale ID reuse bugs.
- Add a resource registry with diagnostics for leaks, reuse, and outstanding references.
- Add `reload` hooks for materials and shaders during authoring or editor workflows.

Why it matters:

- Stale resource IDs are a common source of subtle bugs in engines with many asset types.

### 8. Improve mesh and geometry streaming

Current observations:

- `GeometryPool` is fixed-size and preallocated. This is efficient for small to medium scenes, but it is not a fully scalable asset system.
- Meshes no longer used remain physically allocated until manually freed.

Recommended changes:

- Add mesh streaming so large assets can be uploaded lazily or on demand.
- Add a pool compaction strategy or defragmentation pass for long-running scenes.
- Consider separate pools for static and dynamic meshes.
- Support mesh re-upload or reallocation when resizing the pool.

Why it matters:

- A fixed pool is fine for prototypes, but real-world scenes need streaming and reuse strategy.

### 9. Add a richer texture system

Current observations:

- `Texture` supports basic 2D and array textures, but there is no broader texture manager API beyond ID storage.

Recommended changes:

- Add cube maps and cubemap arrays.
- Add SRGB/linear handling and format-aware upload paths.
- Add texture compression support or at least a standard pipeline for compressed formats.
- Add a texture atlas packing system that is exposed as part of the engine API.

Why it matters:

- This is critical for many modern rendering features and improves runtime flexibility.

---

## Functionality additions that would make the engine more complete

### 10. Add lighting and material pipeline features

Current functionality is still largely a forward shaded, unlit or simple-shaded material model. The engine would benefit from adding:

- Directional light support
- Point lights and spot lights
- Shadow map rendering
- PBR material properties (metallic/roughness workflows)
- Ambient lighting and IBL approximations

Recommended implementation path:

- Add a structured light buffer as a uniform block or UBO.
- Add standard material properties to `MaterialEntry` or a separate material data table.
- Add light culling by tile or cluster, if scene complexity justifies it.

### 11. Add post-processing and screen-space effects

The current renderer clears and draws directly, but it has no post-process pipeline.

Potential features:

- HDR tone mapping
- Bloom
- FXAA / TAA
- Depth-of-field
- SSAO
- Fog and atmospheric scattering

Recommended architecture:

- Add a framebuffer chain for post-processing passes.
- Create a framebuffer stack or render target manager.
- Add a post-process material system that consumes the previous pass as a texture.

### 12. Add animation and skinned mesh support

The engine currently treats objects as static transforms with instanced mesh rendering. There is no animation system.

Recommended additions:

- Bone transforms and skinning matrices
- Skeletal animation data structures
- Dual quaternion or matrix palette support
- GPU skinning pass or CPU-skinning fallback

### 13. Add particle and volumetric systems

Useful extensions:

- Sprite-based particles
- Billboards
- Transparent additive blending passes
- GPU particle simulation
- Smoke/fog layering

### 14. Add editor and debugging utilities

A robust engine should expose debugging support:

- Wireframe mode
- Bounding volume visualization
- Draw call statistics
- Object counts and material counts
- Frustum visualization
- GPU memory usage and pool stats

This would likely be very useful for tuning and validating the engine during development.

---

## API and architecture improvements

### 15. Separate high-level scene logic from low-level render submission

Current observations:

- The renderer owns a large amount of logic: scene management, frustum culling, batching, drawing, and pipeline state.
- This makes the code harder to evolve into a more complex engine.

Recommended refactor:

- Split the renderer into a `SceneSystem`, `RenderSubmissionSystem`, and `RenderBackend`.
- Keep the public API stable while making subsystems independently testable.
- Move render-common logic into a dedicated module rather than embedding everything in `Renderer`.

### 16. Add a proper engine configuration layer

Status: [x] Implemented

Current observations:

- Rendering configuration is scattered in `Renderer::new()` and `resize()`.
- Feature toggles are ad hoc.

Recommended changes:

- Introduce a `RenderConfig`/`EngineConfig` struct.
- Add settings for:
  - MSAA
  - max object count
  - transform buffer region count
  - culling strategy
  - MDI strategy
  - shadow resolution
  - post-processing enable flags

### 17. Improve engine testing and validation

Current codebase appears to be implementation-focused and could benefit from better regression coverage.

Suggested additions:

- Shader compile smoke tests
- Geometry pool allocation tests
- Scene dirty state tests
- Frustum culling correctness tests
- Transform buffer correctness checks
- Render pipeline hashing tests

This should be treated as a core engine concern, not something only left to demo apps.

---

## Recommended implementation roadmap

### Phase 1: Stabilize and optimize the current renderer

1. Reuse frame scratch buffers and reduce temporary allocations.
2. Harden resource lifecycle tracking and ID generation.
3. Refine culling and batching logic to avoid full-scene scans when possible.
4. Improve pipeline-state caching and uniform binding.

### Phase 2: Add core engine features

1. Add a proper light system.
2. Add shadow map support.
3. Add post-process framebuffer chain.
4. Expand material data model beyond the current simple pipeline/material API.

### Phase 3: Scale to larger worlds and more content

1. Add spatial partitioning and streaming mesh support.
2. Add texture atlases and richer texture formats.
3. Improve MDI backend detection and native GL path support.
4. Tune and benchmark against visual and performance budgets.

### Phase 4: Turn the engine into a production-grade foundation

1. Add debug tooling and profiling metrics.
2. Add editor-friendly resource management.
3. Add advanced rendering systems such as particles, fog, and animation.
4. Add a more formal public API and documentation set for engine users.

---

## Concrete next changes to prioritize

If this were the next 3–5 implementation steps, I would prioritize:

1. Frame scratch buffer reuse and temporary allocation elimination.
2. Spatial culling / visibility partitioning.
3. Render pass abstraction and clearer pipeline ordering.
4. Lighting foundation with a uniform light buffer.
5. Material and texture system expansion for modern rendering workflows.

These changes would give the engine the highest return in both performance and capability without requiring a full rewrite.

---

## Summary

The engine already demonstrates several good architectural decisions: persistent GPU buffers, a dirty-scene optimization, indirect draw preparation, and a pipeline cache. The biggest opportunities now are not in a complete redesign; they are in reducing per-frame overhead, improving visibility management, and expanding the engine from a simple forward renderer into a more complete real-time rendering foundation.

The best next move is to optimize the existing renderer core first, then layer in lighting, post-processing, and resource systems once the base pipeline is stable and fast.
