use glow::{Context, HasContext};
use rl_core::failure;

pub struct Texture {
    handle: <Context as HasContext>::Texture,
}
impl Texture {
    pub fn handle(&self) -> <Context as HasContext>::Texture {
        self.handle
    }

    pub fn new<P>(gl: &glow::Context, parameters: P) -> Result<Self, failure::Error>
    where
        P: FnOnce(&glow::Context, <Context as HasContext>::Texture) -> Result<(), failure::Error>,
    {
        let handle = unsafe { gl.create_texture().map_err(failure::err_msg)? };
        (parameters)(gl, handle)?;

        Ok(Self { handle })
    }

    pub unsafe fn bind(&self, gl: &glow::Context, slot: u32) {
        gl.active_texture(slot);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.handle));
    }
}
