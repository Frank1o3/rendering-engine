note i get huge fps drops when i move and it needs to then generate new chucks it means currently its not respecting the limit to were their is 2 threads one is rendering the other is main after that the rest but one can be used so the os can have one thread for its use like that on mobile devices don't instantly overheat also the fps counter is not accurate  ever since we moved rendering to its own thread

Three things gate almost everything else you'd want, and they line up with what's in the diagram: **texture support**, **render targets (FBOs)**, and **compute shader support**. Skybox and the two texture styles you described are the two you asked about directly, so here's the concrete shape of each.

## Texture support — both loading styles

Both styles share one primitive: a `Texture` wrapper (new `engine/src/texture.rs`) holding a `glow::Texture` handle and its GL target. What differs is how you populate it and how meshes reference it.

**Atlas style (single big image, UV offset moves per tile)** — this is closer to what you described second, "moving across the image." One `Texture2D`, and a small helper that hands back a UV rectangle for a given tile index:
```rust
pub struct TextureAtlas {
    pub texture: Texture,
    pub tile_size: (u32, u32),
    pub grid: (u32, u32), // tiles across, tiles down
}
impl TextureAtlas {
    pub fn uv_rect(&self, tile_x: u32, tile_y: u32) -> [f32; 4] { /* u_min, v_min, u_max, v_max */ }
}
```
Your mesh generator (e.g. the voxel mesher) calls `uv_rect` per face and bakes those UVs straight into `Vertex`. Cheap, one texture bind for the whole scene — but needs a pixel of padding per tile to avoid mip bleeding at distance, which matters for your voxel terrain specifically.

**Per-face / individual-image style (Minecraft's actual approach)** — each block face is its own source image, combined at *load time* into either an atlas (packed via a rect-packing pass) or a `GL_TEXTURE_2D_ARRAY` where each image becomes one layer:
```rust
pub fn load_array_from_paths(gl: Arc<glow::Context>, paths: &[&Path]) -> (Texture, Vec<u32>)
// returns the array texture + a layer index per input path, in call order
```
The array route is actually the better fit for your block-face case: no padding/bleeding math, uniform mipmaps per layer, and the vertex only needs a `layer: f32` instead of computed UV offsets — sample with `texture(sampler, vec3(uv, layer))`. The tradeoff is every image must be the same resolution, which is true for practically all voxel-style texture sets anyway.

Either way this touches `Vertex` (needs a UV field — currently `Vertex` has no UV channel at all, so this is the one unavoidable breaking change), `GeometryPool`'s attribute pointers, both `.vert` shaders, and `MaterialEntry` (gets a `texture: Option<glow::Texture>` so a draw's bound texture falls out of its existing `MaterialId`, no `RenderCommand` change needed).

## Skybox — matching your three-function shape

Your instinct to route it through the existing shader loader is right, and keeps this small:

```rust
// 1. Build the pipeline — reuses load_shader/load_shaders_from_include_dir internally
pub fn create_skybox(&mut self, vs_src: &str, fs_src: &str, gs_src: Option<&str>) -> SkyboxId {
    let shader = self.load_shader(vs_src, gs_src, fs_src); // same call path as your other shaders
    // registers a dedicated PipelineState: depth_func = LEQUAL, depth_write = false, cull = None
    // no vertex/index buffers needed — engine draws a built-in fullscreen triangle
}

// 2. Toggle it — render() checks this before Pass 1
pub fn enable_skybox(&mut self, id: SkyboxId);
pub fn disable_skybox(&mut self);

// 3. Convenience tint — thin wrapper over your existing upload_shader_vec3
pub fn set_skybox_color(&mut self, id: SkyboxId, color: glam::Vec3);
```

Inside `render()`, the skybox draws first (before Pass 1), fullscreen triangle, no `Scene`/`FrameData.commands` involvement at all — the engine auto-uploads the inverse view-projection matrix each frame (same way `uVP` already gets set) so the user's fragment shader can reconstruct a view ray and sample their cubemap or do fully custom procedural sky math. `set_skybox_color` just gives them a convenience uniform to blend against — nothing forces them to use it, matching "full control, they don't need to touch the shader for color."

The one real gap for your "geometry shader or compute shader" plan: `ShaderProgram::new` currently only compiles vertex + optional geometry + fragment. A compute-shader skybox (e.g. baking atmospheric scattering into a texture the fragment shader then samples) needs a genuinely new compute pipeline — `glow::COMPUTE_SHADER` compilation, `dispatch_compute`, and image/SSBO bindings. That's the "Compute shaders" box in the diagram — worth doing, but it's an independent, bigger unit of work from the skybox draw path itself, and the vertex+fragment skybox path doesn't need to wait on it.

Want the file-by-file diff plan for the `Vertex` UV change first, since it's the one every other box in that diagram sits downstream of?
