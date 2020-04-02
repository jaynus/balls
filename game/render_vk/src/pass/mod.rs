use crate::RenderContext;
use ash::vk;
use rl_core::{app::ApplicationContext, math::Mat4, GameState};

pub mod debug;
pub mod entities;
pub mod map;
pub mod sparse_sprite;
pub mod spine;

pub trait VkPass {
    fn subpass_dependency(&self, index: u32) -> vk::SubpassDependency;
    fn subpass(&self) -> vk::SubpassDescription;

    unsafe fn rebuild(
        &mut self,
        context: &RenderContext,
        subpass: u32,
    ) -> Result<vk::Pipeline, anyhow::Error>;

    unsafe fn draw(
        &mut self,
        args: RenderArgs<'_>,
        secondary_command_buffers: &mut Vec<vk::CommandBuffer>,
    ) -> Result<(), anyhow::Error>;
}

pub struct RenderArgs<'a> {
    pub app_context: &'a mut ApplicationContext,
    pub render_context: &'a mut RenderContext,
    pub command_buffer: ash::vk::CommandBuffer,
    pub state: &'a GameState,
    pub current_frame: usize,
}

#[repr(C, align(16))]
pub struct Environment {
    view_proj: Mat4,
}
