use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use std::sync::Arc;

/// A vertex in pixel-space UI coordinates. `(0, 0)` is the top-left of the
/// window, and colors use 8-bit RGBA channels.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2],
    pub color: [u8; 4],
}

/// Per-frame 2D geometry, independent of scene meshes and camera transforms.
#[derive(Clone, Debug, Default)]
pub struct UiDrawList {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
}

impl UiDrawList {
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Add a solid screen-space rectangle. Coordinates start at the top-left.
    pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: [u8; 4]) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            UiVertex {
                position: [x, y],
                color,
            },
            UiVertex {
                position: [x + width, y],
                color,
            },
            UiVertex {
                position: [x + width, y + height],
                color,
            },
            UiVertex {
                position: [x, y + height],
                color,
            },
        ]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

/// Dedicated 2D overlay renderer. It owns its buffers and shader, so UI
/// geometry does not share scene meshes, instance transforms, or camera VP.
pub(crate) struct UiRenderer {
    gl: Arc<glow::Context>,
    program: glow::Program,
    viewport_location: Option<glow::UniformLocation>,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
}

impl UiRenderer {
    pub(crate) fn new(gl: Arc<glow::Context>) -> Self {
        const VERTEX_SHADER: &str = r#"#version 320 es
precision highp float;
layout(location = 0) in vec2 aPosition;
layout(location = 1) in vec4 aColor;
uniform vec2 uViewport;
out vec4 vColor;
void main() {
    vec2 clip = vec2(aPosition.x / uViewport.x * 2.0 - 1.0,
                     1.0 - aPosition.y / uViewport.y * 2.0);
    gl_Position = vec4(clip, 0.0, 1.0);
    vColor = aColor;
}"#;
        const FRAGMENT_SHADER: &str = r#"#version 320 es
precision highp float;
in vec4 vColor;
out vec4 FragColor;
void main() { FragColor = vColor; }
"#;

        unsafe {
            let vertex = Self::compile(&gl, glow::VERTEX_SHADER, VERTEX_SHADER);
            let fragment = Self::compile(&gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER);
            let program = gl.create_program().expect("Failed to create UI program");
            gl.attach_shader(program, vertex);
            gl.attach_shader(program, fragment);
            gl.link_program(program);
            gl.delete_shader(vertex);
            gl.delete_shader(fragment);
            if !gl.get_program_link_status(program) {
                let log = gl.get_program_info_log(program);
                gl.delete_program(program);
                panic!("Failed to link UI program: {log}");
            }

            let vao = gl.create_vertex_array().expect("Failed to create UI VAO");
            let vbo = gl.create_buffer().expect("Failed to create UI VBO");
            let ebo = gl.create_buffer().expect("Failed to create UI EBO");
            gl.bind_vertex_array(Some(vao));

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let stride = std::mem::size_of::<UiVertex>() as i32;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 4, glow::UNSIGNED_BYTE, true, stride, 8);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
            gl.bind_vertex_array(None);

            let viewport_location = gl.get_uniform_location(program, "uViewport");
            Self {
                gl,
                program,
                viewport_location,
                vao,
                vbo,
                ebo,
            }
        }
    }

    unsafe fn compile(gl: &glow::Context, kind: u32, source: &str) -> glow::Shader {
        unsafe {
            let shader = gl.create_shader(kind).expect("Failed to create UI shader");
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                panic!("Failed to compile UI shader: {log}");
            }
            shader
        }
    }

    pub(crate) fn draw(&self, list: &UiDrawList, width: i32, height: i32) {
        if list.is_empty() || width <= 0 || height <= 0 {
            return;
        }

        unsafe {
            self.gl.disable(glow::DEPTH_TEST);
            self.gl.disable(glow::CULL_FACE);
            self.gl.depth_mask(false);
            self.gl.enable(glow::BLEND);
            self.gl
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.gl.use_program(Some(self.program));
            self.gl
                .uniform_2_f32(self.viewport_location.as_ref(), width as f32, height as f32);

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&list.vertices),
                glow::STREAM_DRAW,
            );
            self.gl
                .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            self.gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                bytemuck::cast_slice(&list.indices),
                glow::STREAM_DRAW,
            );
            self.gl.draw_elements(
                glow::TRIANGLES,
                list.indices.len() as i32,
                glow::UNSIGNED_INT,
                0,
            );
            self.gl.bind_vertex_array(None);
            self.gl.disable(glow::BLEND);
        }
    }
}

impl Drop for UiRenderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_buffer(self.ebo);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_program(self.program);
        }
    }
}
