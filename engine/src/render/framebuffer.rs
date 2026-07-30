use glow::HasContext;
use std::sync::Arc;

pub struct Framebuffer {
    gl: Arc<glow::Context>,
    pub fbo: glow::Framebuffer,
    pub color_texture: glow::Texture,
    pub depth_renderbuffer: Option<glow::Renderbuffer>,
    pub depth_texture: Option<glow::Texture>,
    pub width: u32,
    pub height: u32,
}

impl Framebuffer {
    pub fn new(
        gl: Arc<glow::Context>,
        width: u32,
        height: u32,
        with_depth_texture: bool,
    ) -> Result<Self, String> {
        unsafe {
            let fbo = gl.create_framebuffer().map_err(|e| e.to_string())?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));

            let color_texture = gl.create_texture().map_err(|e| e.to_string())?;
            gl.bind_texture(glow::TEXTURE_2D, Some(color_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(color_texture),
                0,
            );

            let (depth_renderbuffer, depth_texture) = if with_depth_texture {
                let depth_tex = gl.create_texture().map_err(|e| e.to_string())?;
                gl.bind_texture(glow::TEXTURE_2D, Some(depth_tex));
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::DEPTH_COMPONENT24 as i32,
                    width as i32,
                    height as i32,
                    0,
                    glow::DEPTH_COMPONENT,
                    glow::UNSIGNED_INT,
                    glow::PixelUnpackData::Slice(None),
                );
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::DEPTH_ATTACHMENT,
                    glow::TEXTURE_2D,
                    Some(depth_tex),
                    0,
                );
                (None, Some(depth_tex))
            } else {
                let rbo = gl.create_renderbuffer().map_err(|e| e.to_string())?;
                gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rbo));
                gl.renderbuffer_storage(
                    glow::RENDERBUFFER,
                    glow::DEPTH24_STENCIL8,
                    width as i32,
                    height as i32,
                );
                gl.framebuffer_renderbuffer(
                    glow::FRAMEBUFFER,
                    glow::DEPTH_STENCIL_ATTACHMENT,
                    glow::RENDERBUFFER,
                    Some(rbo),
                );
                (Some(rbo), None)
            };

            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            if status != glow::FRAMEBUFFER_COMPLETE {
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                return Err(format!("Framebuffer incomplete status: {:#x}", status));
            }

            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            Ok(Self {
                gl,
                fbo,
                color_texture,
                depth_renderbuffer,
                depth_texture,
                width,
                height,
            })
        }
    }

    pub fn bind(&self) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.viewport(0, 0, self.width as i32, self.height as i32);
        }
    }

    pub fn unbind(&self, default_width: i32, default_height: i32) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.gl.viewport(0, 0, default_width, default_height);
        }
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.color_texture);
            if let Some(depth_tex) = self.depth_texture {
                self.gl.delete_texture(depth_tex);
            }
            if let Some(rbo) = self.depth_renderbuffer {
                self.gl.delete_renderbuffer(rbo);
            }
        }
    }
}
