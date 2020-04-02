use crate::{
    math::{self, geometry::Ray, Mat4, Vec2, Vec3},
    transform,
};
use legion::prelude::*;

#[derive(Clone, Copy)]
pub struct CameraQueryResult {
    pub camera: Camera,
    pub translation: transform::Translation,
    pub scale: transform::Scale,
}

pub type CameraQueryFn = Box<dyn FnMut(&World) -> CameraQueryResult + Send + Sync>;

pub fn make_camera_query() -> CameraQueryFn {
    let camera_query = <(
        Read<Camera>,
        Read<transform::Translation>,
        Read<transform::Scale>,
    )>::query();

    Box::new(move |world: &World| {
        let camera = camera_query.iter(&world).next().unwrap();
        CameraQueryResult {
            camera: (*camera.0),
            translation: (*camera.1),
            scale: (*camera.2),
        }
    })
}

#[derive(Debug, Copy, Clone)]
pub struct Camera {
    pub projection: Mat4,
    pub dimensions: Vec2,
}
impl Camera {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            //projection: math::projection::orthographic_vk(0.0, width, height, 0.0, 0.1, 100.0),
            projection: math::projection::orthographic_vk(
                -(width / 2.0),
                width / 2.0,
                height / 2.0,
                -(height / 2.0),
                0.1,
                100.0,
            ),
            dimensions: Vec2::new(width, height),
        }
    }

    /*
    pub fn matrix(&self, translation: &transform::Translation, scale: &transform::Scale) -> Mat4 {
        let view = Mat4::from_translation(**translation) * Mat4::identity();
        self.projection * view
    }*/

    pub fn matrix(&self, translation: &transform::Translation, scale: transform::Scale) -> Mat4 {
        let view = math::Mat4::look_at(
            Vec3::new(translation.x, translation.y, 5.0),
            Vec3::new(translation.x, translation.y, 4.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        (self.projection * Mat4::from_scale(*scale)) * view
    }

    pub fn unproject(
        &self,
        screen_position: Vec3,
        (translation, scale): (&transform::Translation, &transform::Scale),
    ) -> Vec3 {
        let mut matrix = self.matrix(translation, *scale);
        matrix.inverse();

        let mut tmp = screen_position.into_homogeneous_point();
        tmp.x /= self.dimensions.x;
        tmp.y /= self.dimensions.y;
        tmp.x = tmp.x * 2.0 - 1.0;
        tmp.y = tmp.y * 2.0 - 1.0;

        let mut res = matrix * tmp;

        res /= res.w;
        res.z = translation.z;

        res.xyz()
    }

    pub fn screen_ray(
        &self,
        screen_position: Vec2,
        (translation, scale): (&transform::Translation, &transform::Scale),
    ) -> Ray {
        let screen_x = 2.0 * screen_position.x / self.dimensions.x - 1.0;
        let screen_y = 2.0 * screen_position.y / self.dimensions.y - 1.0;

        let mut matrix = self.matrix(translation, *scale);
        matrix.inverse();

        let near = Vec3::new(screen_x, screen_y, 0.0);
        let far = Vec3::new(screen_x, screen_y, 1.0);

        let near_t = matrix * near.into_homogeneous_point();
        let far_t = matrix * far.into_homogeneous_point();

        Ray {
            origin: near_t.xyz(),
            direction: (far_t - near_t).normalized().xyz(),
        }
    }
}
