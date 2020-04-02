use crate::{GlPass, RenderArgs, ShaderProgram};

pub struct DebugLinesPass {
    shader: ShaderProgram,
}
impl DebugLinesPass {
    pub fn new(args: &mut RenderArgs<'_>) -> Result<Box<dyn GlPass>, anyhow::Error> {
        let gl = &args.render_context.gl;
        let shader = ShaderProgram::compile_graphics(
            gl,
            "assets/shaders/",
            include_str!("../../../assets/shaders/debug.vert"),
            include_str!("../../../assets/shaders/debug.frag"),
        )?;

        args.state
            .resources
            .insert(rl_core::debug::DebugLines::default());

        Ok(Box::new(Self { shader }))
    }
}
impl GlPass for DebugLinesPass {
    unsafe fn draw(&mut self, args: RenderArgs<'_>) -> Result<(), anyhow::Error> {
        let gl = &args.render_context.gl;

        self.shader.bind(gl);

        Ok(())
    }
}
