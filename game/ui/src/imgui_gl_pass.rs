#![allow(clippy::cast_possible_wrap)]
use crate::ImguiContextLock;
use imgui_winit_support::WinitPlatform;
use rl_core::{legion::prelude::*, winit::window::Window, ScreenDimensions};
use rl_render_glow::{
    buffer::{Buffer, BufferUsage},
    glow::{self, HasContext},
    state::RenderState,
    texture::Texture,
    vertex::{VertexAttrib, VertexDecl},
    GlPass, RenderArgs, ShaderProgram,
};
use rl_render_pod::std140::*;
use std::sync::Arc;

pub struct ImguiVertexDecl;
impl VertexDecl for ImguiVertexDecl {
    fn desc() -> Vec<VertexAttrib> {
        vec![
            VertexAttrib::from_std140::<vec2>("aPos"),
            VertexAttrib::from_std140::<vec2>("aUv"),
            VertexAttrib::new(
                "aColor",
                4,
                glow::UNSIGNED_BYTE,
                (std::mem::size_of::<u8>() * 4) as u32,
                true,
            ),
        ]
    }
}

pub struct ImguiPass {
    shader: ShaderProgram,
    vbo: Buffer,
    ibo: Buffer,
    state: RenderState,

    font_atlas: Texture,
}
impl ImguiPass {
    pub fn new(args: &mut RenderArgs<'_>) -> Result<Box<dyn GlPass>, anyhow::Error> {
        let gl = &args.render_context.gl;

        let shader = ShaderProgram::compile_graphics(
            gl,
            "assets/shaders/",
            include_str!("../../assets/shaders/imgui.vert"),
            include_str!("../../assets/shaders/imgui.frag"),
        )?;

        let (_platform, _window, dimensions, imgui_lock) = <(
            Read<WinitPlatform>,
            Read<Arc<Window>>,
            Read<ScreenDimensions>,
            Read<ImguiContextLock>,
        )>::fetch(&args.state.resources);

        let imgui = &mut imgui_lock.lock().unwrap().context;

        imgui.io_mut().display_size = [dimensions.size.width as f32, dimensions.size.height as f32];
        imgui.io_mut().display_framebuffer_scale = [dimensions.dpi as f32, dimensions.dpi as f32];

        imgui
            .io_mut()
            .backend_flags
            .insert(imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);

        imgui.fonts().tex_id = imgui::TextureId::from(0);

        let font_atlas = {
            let mut atlas = imgui.fonts();
            let src_texture = atlas.build_rgba32_texture();
            let texture = Texture::new(gl, |gl, texture| unsafe {
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::LINEAR as _,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::LINEAR as _,
                );
                gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);

                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    src_texture.width as i32,
                    src_texture.height as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    Some(src_texture.data),
                );
                Ok(())
            })?;
            atlas.tex_id = imgui::TextureId::from(texture.handle() as usize);

            texture
        };

        Ok(Box::new(Self {
            shader,
            state: RenderState {
                blend: true,
                cull_face: false,
                depth_test: false,
                scissor_test: true,

                ..RenderState::default()
            },
            font_atlas,
            vbo: Buffer::new(gl, BufferUsage::Array, None)?,
            ibo: Buffer::new(gl, BufferUsage::ElementArray, None)?,
        }))
    }
}
impl GlPass for ImguiPass {
    unsafe fn draw(&mut self, args: RenderArgs<'_>) -> Result<(), anyhow::Error> {
        let gl = &args.render_context.gl;

        self.state.apply(gl);
        self.shader.bind(gl);

        self.font_atlas.bind(gl, glow::TEXTURE0);

        Ok(())
    }
}
