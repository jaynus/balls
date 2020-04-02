use crate::math::{Vec3i, Vec3iProxy};

/// The most basic encoder, which strictly flattens the 3d space into 1d coordinates in a linear fashion.
/// This encoder is optimal for storage space, but not for traversal or iteration.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FlatEncoder {
    #[serde(with = "Vec3iProxy")]
    dimensions: Vec3i,
}
impl FlatEncoder {
    pub fn from_dimensions(dimensions: Vec3i) -> Self {
        Self { dimensions }
    }

    #[inline]
    #[allow(clippy::cast_sign_loss)]
    pub fn encode(&self, coord: Vec3i) -> usize {
        ((coord.z * self.dimensions.x * self.dimensions.y)
            + (coord.y * self.dimensions.x)
            + coord.x) as usize
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn decode(&self, idx: usize) -> Vec3i {
        let idx = idx as i32;
        let z = idx / (self.dimensions.x * self.dimensions.y);
        let idx = idx - (z * self.dimensions.x * self.dimensions.y);
        let y = idx / self.dimensions.x;
        let x = idx % self.dimensions.x;

        Vec3i::new(x, y, z)
    }

    #[allow(clippy::cast_sign_loss)]
    pub fn allocation_size(dimensions: Vec3i) -> usize {
        (dimensions.x * dimensions.y * dimensions.z) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_flat_encoder() {
        let test_coords = [
            Vec3i::new(0, 0, 0),
            Vec3i::new(0, 0, 1),
            Vec3i::new(123, 123, 4),
            Vec3i::new(0, 1, 2),
            Vec3i::new(1, 2, 3),
            Vec3i::new(55, 10, 1),
        ];

        let encoder = FlatEncoder::from_dimensions(Vec3i::new(128, 128, 5));

        for coord in &test_coords {
            let index = encoder.encode(*coord);
            let decoded = encoder.decode(index);
            assert_eq!(decoded, *coord);
            println!("{:?} = {:?}", *coord, decoded);
        }
    }
}
