use crate::{
    components::{DimensionsComponent, PositionComponent},
    data::CollisionKind,
    legion::prelude::*,
    math::Vec3i,
    shrinkwrap::Shrinkwrap,
};

impl rstar::Point for PositionComponent {
    type Scalar = i32;
    const DIMENSIONS: usize = 3;

    fn generate(generator: impl Fn(usize) -> Self::Scalar) -> Self {
        PositionComponent::new(Vec3i::new(generator(0), generator(1), generator(2)))
    }
    fn nth(&self, index: usize) -> Self::Scalar {
        match index {
            0 => self.coord.x,
            1 => self.coord.y,
            2 => self.coord.z,
            _ => unimplemented!(),
        }
    }
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        match index {
            0 => &mut self.coord.x,
            1 => &mut self.coord.y,
            2 => &mut self.coord.z,
            _ => unimplemented!(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpatialMapEntry {
    pub entity: Entity,
    pub collision: CollisionKind,
    rect: rstar::primitives::Rectangle<PositionComponent>,
}
impl PartialEq for SpatialMapEntry {
    fn eq(&self, rhv: &Self) -> bool {
        self.entity.eq(&rhv.entity) && self.rect.eq(&rhv.rect)
    }
}
impl SpatialMapEntry {
    pub fn with_rect(
        entity: Entity,
        rect: rstar::primitives::Rectangle<PositionComponent>,
        collision: CollisionKind,
    ) -> Self {
        Self {
            entity,
            rect,
            collision,
        }
    }
    pub fn new(
        entity: Entity,
        position: &PositionComponent,
        dimensions: &DimensionsComponent,
    ) -> Self {
        Self::with_rect(
            entity,
            rstar::primitives::Rectangle::from_corners(
                *position,
                (**position
                    + ((dimensions.as_tiles() - Vec3i::new(1, 1, 1)) * Vec3i::new(1, 1, -1)))
                .into(),
            ),
            dimensions.collision(),
        )
    }

    pub fn new_single(entity: Entity, coord: Vec3i, collision: CollisionKind) -> Self {
        Self {
            entity,
            rect: rstar::primitives::Rectangle::from_corners(coord.into(), coord.into()),
            collision,
        }
    }

    pub fn aabb(&self) -> rstar::AABB<PositionComponent> {
        rstar::AABB::from_corners(self.rect.lower(), self.rect.upper())
    }

    pub fn rectangle(&self) -> rstar::primitives::Rectangle<PositionComponent> {
        self.rect
    }

    pub fn collision(&self) -> CollisionKind {
        self.collision
    }

    pub fn position(&self) -> Vec3i {
        *self.rect.lower()
    }

    pub fn dimensions(&self) -> Vec3i {
        (*self.rect.upper() - *self.rect.lower()) + Vec3i::new(1, 1, 1)
    }
}
impl rstar::RTreeObject for SpatialMapEntry {
    type Envelope = rstar::AABB<PositionComponent>;

    #[inline]
    fn envelope(&self) -> Self::Envelope {
        self.rect.envelope()
    }
}
impl rstar::PointDistance for SpatialMapEntry {
    #[inline]
    fn distance_2(&self, point: &PositionComponent) -> i32 {
        self.rect.distance_2(point)
    }

    #[inline]
    fn contains_point(&self, point: &PositionComponent) -> bool {
        self.rect.contains_point(point)
    }
}

#[derive(Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
pub struct SpatialMap(pub rstar::RTree<SpatialMapEntry>);

#[derive(Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
pub struct StaticSpatialMap(pub rstar::RTree<SpatialMapEntry>);
