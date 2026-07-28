// src/renderer_setup.rs

use std::{ffi::CString, sync::Arc};

use glow::Context;
use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext, Version},
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, SwapInterval},
};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, window::Window};

use rendering_engine::{
    engine::Renderer,
    frame_data::FrameData,
    pipeline::{BlendFactor, CullMode, DepthFunc, PipelineState},
    triple_buffer::new_triple_buffer,
};

use crate::{
    meshes::{
        create_button_quad_mesh, create_collectible_mesh, create_cube_mesh, create_quad_mesh,
    },
    shaders::SHADERS,
    state::{Assets, DemoState, Keys},
};

use glam::Vec3;

use std::{collections::HashMap, time::Instant};

use crate::touch::TouchKind;

pub fn create_demo_state(
    event_loop: &ActiveEventLoop,
    template: ConfigTemplateBuilder,
) -> DemoState {
    let window_attributes = Window::default_attributes()
        .with_title("Rendering Engine — OpenGL ES 3.2 3D Demo")
        .with_inner_size(PhysicalSize::new(1280, 720))
        .with_resizable(false);

    let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

    let (window, gl_config) = display_builder
        .build(event_loop, template, |configs| {
            configs
                .reduce(|a, b| {
                    if b.num_samples() > a.num_samples() {
                        b
                    } else {
                        a
                    }
                })
                .unwrap()
        })
        .expect("Failed to create display");

    let window = Arc::new(window.expect("Failed to create window"));

    let display = gl_config.display();

    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(Some(Version { major: 3, minor: 2 })))
        .build(Some(window.window_handle().unwrap().as_raw()));

    let not_current = unsafe {
        display
            .create_context(&gl_config, &context_attributes)
            .expect("Failed to create GL context")
    };

    let attrs = window.build_surface_attributes(Default::default()).unwrap();

    let gl_surface = unsafe { display.create_window_surface(&gl_config, &attrs).unwrap() };

    let gl_context = not_current.make_current(&gl_surface).unwrap();

    gl_surface
        .set_swap_interval(&gl_context, SwapInterval::DontWait)
        .unwrap();

    let gl = unsafe {
        Context::from_loader_function(|sym| display.get_proc_address(&CString::new(sym).unwrap()))
    };

    let (write_handle, read_handle) = new_triple_buffer::<FrameData>();

    let mut renderer = Renderer::new(gl, read_handle);

    //
    // Meshes
    //

    let cube_mesh = renderer.load_mesh(create_cube_mesh());
    let quad_mesh = renderer.load_mesh(create_quad_mesh());
    let button_quad_mesh = renderer.load_mesh(create_button_quad_mesh());
    let collectible_mesh = renderer.load_mesh(create_collectible_mesh());

    //
    // Shaders
    //

    let shader_map = renderer
        .load_shaders_from_include_dir(&SHADERS)
        .expect("Failed to load shaders");

    let lit_shader = *shader_map.get("lit").expect("Missing lit shader");
    let ui_shader = *shader_map.get("ui").expect("Missing ui shader");

    //
    // Materials
    //

    let lit_material =
        renderer.create_material(lit_shader, PipelineState::default_opaque(lit_shader.0));

    let ui_pipeline = PipelineState {
        shader_id: ui_shader.0,
        cull_mode: CullMode::None,
        depth_test: true,
        depth_write: false,
        depth_func: DepthFunc::Less,
        blend_enabled: true,
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
    };

    let ui_material = renderer.create_material(ui_shader, ui_pipeline);

    DemoState {
        window,
        gl_context,
        gl_surface,

        renderer,
        write_handle,

        assets: Assets {
            cube_mesh,
            quad_mesh,
            button_quad_mesh,
            collectible_mesh,

            lit_material,
            ui_material,

            lit_shader,
        },

        width: 1280,
        height: 720,

        camera_pos: Vec3::new(0.0, 1.0, 5.0),
        camera_yaw: 0.0,
        camera_pitch: 0.0,

        keys: Keys::default(),
        cursor_grabbed: false,

        dyn_angle: 0.0,

        last_frame: Instant::now(),

        current_fps: 0.0,
        frame_count: 0,
        last_fps_update: Instant::now(),

        touches: HashMap::<u64, TouchKind>::new(),

        game: crate::state::GameState::new(), // placeholder, will be overwritten in init
    }
}
