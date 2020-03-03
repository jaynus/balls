use glow::HasContext;
use std::ops::{BitAnd, BitOr};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RenderState {
    pub blend: bool,
    pub cull_face: bool,
    pub depth_test: bool,
    pub scissor_test: bool,

    pub blend_equation: u32,
    pub blend_func: (u32, u32),
    pub polygon_mode: (u32, u32),
}
impl Default for RenderState {
    fn default() -> Self {
        Self {
            blend: false,
            cull_face: false,
            depth_test: false,
            scissor_test: false,

            blend_equation: glow::FUNC_ADD,
            blend_func: (glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA),
            polygon_mode: (glow::FRONT_AND_BACK, glow::FILL),
        }
    }
}
impl RenderState {
    #[allow(clippy::cast_sign_loss)]
    pub fn save(gl: &glow::Context) -> Self {
        unsafe {
            Self {
                blend: gl.is_enabled(glow::BLEND),
                cull_face: gl.is_enabled(glow::CULL_FACE),
                depth_test: gl.is_enabled(glow::DEPTH_TEST),
                scissor_test: gl.is_enabled(glow::SCISSOR_TEST),

                blend_equation: gl.get_parameter_i32(glow::BLEND_EQUATION) as u32,
                blend_func: (glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA),
                polygon_mode: (glow::FRONT_AND_BACK, glow::FILL),
            }
        }
    }

    pub fn apply(&self, gl: &glow::Context) {
        unsafe {
            if self.blend {
                gl.enable(glow::BLEND);
            } else {
                gl.disable(glow::BLEND);
            }

            if self.cull_face {
                gl.enable(glow::CULL_FACE);
            } else {
                gl.disable(glow::CULL_FACE);
            }
            if self.depth_test {
                gl.enable(glow::DEPTH_TEST);
            } else {
                gl.disable(glow::DEPTH_TEST);
            }
            if self.scissor_test {
                gl.enable(glow::SCISSOR_TEST);
            } else {
                gl.disable(glow::SCISSOR_TEST);
            }

            gl.blend_equation(self.blend_equation);
            gl.blend_func(self.blend_func.0, self.blend_func.1);
            gl.polygon_mode(self.polygon_mode.0, self.polygon_mode.1);
        }
    }
}
impl BitOr for RenderState {
    type Output = Self;

    fn bitor(self, _rhs: Self) -> Self {
        unimplemented!()
    }
}
impl BitAnd for RenderState {
    type Output = Self;

    fn bitand(self, _rhs: Self) -> Self {
        unimplemented!()
    }
}
