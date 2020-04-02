use crate::math::Vec3;
use rl_render_pod::color::Color;
use smallvec::SmallVec;

#[derive(Default, Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct DebugVert {
    pub start: [f32; 3],
    pub start_color: u32,
    pub end: [f32; 3],
    pub end_color: u32,
}

#[derive(Default, Debug, Clone)]
pub struct DebugLines {
    pub lines: SmallVec<[DebugVert; 1024]>,
}
impl DebugLines {
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Adds a line to be rendered by giving a start and an end position.
    pub fn add_line(&mut self, start: Vec3, end: Vec3, color: Color) {
        self.add_gradient_line(start, end, color, color);
    }

    /// Adds a line to be rendered by giving a start and an end position with separate start and end colors.
    pub fn add_gradient_line(
        &mut self,
        start: Vec3,
        end: Vec3,
        start_color: Color,
        end_color: Color,
    ) {
        self.lines.push(DebugVert {
            start: start.into(),
            start_color: start_color.pack(),
            end: end.into(),
            end_color: end_color.pack(),
        });
    }

    /// Adds multiple lines that form a rectangle to be rendered by giving a Z coordinate, a min and a max position.
    ///
    /// This rectangle is aligned to the XY plane.
    pub fn add_rectangle_2d(&mut self, min: Vec3, max: Vec3, z: f32, color: Color) {
        self.add_line([min.x, min.y, z].into(), [max.x, min.y, z].into(), color);
        self.add_line([min.x, min.y, z].into(), [min.x, max.y, z].into(), color);
        self.add_line([max.x, min.y, z].into(), [max.x, max.y, z].into(), color);
        self.add_line([min.x, max.y, z].into(), [max.x, max.y, z].into(), color);
        self.add_line([min.x, max.y, z].into(), [max.x, max.y, z].into(), color);
    }
}
