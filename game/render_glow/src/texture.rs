use glow::{Context, HasContext};

pub struct Texture {
    handle: <Context as HasContext>::Texture,
}
impl Texture {
    pub fn handle(&self) -> <Context as HasContext>::Texture {
        self.handle
    }

    pub fn new<P>(gl: &glow::Context, parameters: P) -> Result<Self, anyhow::Error>
    where
        P: FnOnce(&glow::Context, <Context as HasContext>::Texture) -> Result<(), anyhow::Error>,
    {
        let handle = unsafe {
            gl.create_texture()
                .map_err(|e| anyhow::anyhow!("Failed to create texture: {:?}", e))?
        };
        (parameters)(gl, handle)?;

        Ok(Self { handle })
    }

    pub unsafe fn bind(&self, gl: &glow::Context, slot: u32) {
        gl.active_texture(slot);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.handle));
    }
}
