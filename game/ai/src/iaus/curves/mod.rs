use std::{
    fmt::Debug,
    ops::{Bound, RangeBounds, RangeInclusive},
};

pub trait Curve<R>: Debug + Send + Sync
where
    R: RangeBounds<f64> + Debug + Send + Sync,
{
    fn range(&self) -> R;
    fn transform(&self, input: f64) -> f64;

    fn normalize(&self, input: f64) -> f64 {
        let range = self.range();

        let start = match range.start_bound() {
            Bound::Unbounded => -std::f64::INFINITY,
            Bound::Included(start) => *start,
            Bound::Excluded(start) => *start + 1.0,
        };

        let end = match range.end_bound() {
            Bound::Unbounded => std::f64::INFINITY,
            Bound::Included(end) => *end,
            Bound::Excluded(end) => *end - 1.0,
        };

        (input - start) / (end - start)
    }
}

#[derive(Debug)]
pub struct Linear<R = RangeInclusive<f64>> {
    pub range: R,
    pub slope: f64,
    pub intercept: f64,
}
impl Default for Linear {
    fn default() -> Self {
        Self {
            range: 0.0..=1.0,
            slope: 1.0,
            intercept: 0.0,
        }
    }
}
impl<R> Linear<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    pub fn new(range: R) -> Self {
        Self {
            range,
            slope: 1.0,
            intercept: 0.0,
        }
    }
}
impl<R> Curve<R> for Linear<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    #[inline]
    fn range(&self) -> R {
        self.range.clone()
    }

    #[inline]
    fn transform(&self, input: f64) -> f64 {
        let input = self.normalize(input);
        (input * self.slope) + self.intercept
    }
}

#[derive(Debug)]
pub struct Exponential<R = RangeInclusive<f64>> {
    pub range: R,
    pub power: f64,
    pub intercept: f64,
}
impl Default for Exponential {
    fn default() -> Self {
        Self {
            range: 0.0..=1.0,
            power: 2.0,
            intercept: 0.0,
        }
    }
}
impl<R> Exponential<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    pub fn new(range: R) -> Self {
        Self {
            range,
            power: 2.0,
            intercept: 0.0,
        }
    }
}
impl<R> Curve<R> for Exponential<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    #[inline]
    fn range(&self) -> R {
        self.range.clone()
    }

    #[inline]
    fn transform(&self, input: f64) -> f64 {
        let input = self.normalize(input);
        input.powf(self.power) + self.intercept
    }
}

#[derive(Debug)]
pub struct Sine<R = RangeInclusive<f64>> {
    pub range: R,
    pub magnitude: f64,
    pub intercept: f64,
}
impl Default for Sine {
    fn default() -> Self {
        Self {
            range: 0.0..=1.0,
            magnitude: 0.5,
            intercept: 0.0,
        }
    }
}
impl<R> Sine<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    pub fn new(range: R) -> Self {
        Self {
            range,
            magnitude: 0.5,
            intercept: 0.0,
        }
    }
}
impl<R> Curve<R> for Sine<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    #[inline]
    fn range(&self) -> R {
        self.range.clone()
    }

    #[inline]
    fn transform(&self, input: f64) -> f64 {
        let input = self.normalize(input);
        (input * std::f64::consts::PI * self.magnitude).sin() + self.intercept
    }
}

#[derive(Debug)]
pub struct Cosine<R = RangeInclusive<f64>> {
    pub range: R,
    pub magnitude: f64,
    pub intercept: f64,
}
impl Default for Cosine {
    fn default() -> Self {
        Self {
            range: 0.0..=1.0,
            magnitude: 0.5,
            intercept: 0.0,
        }
    }
}
impl<R> Cosine<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    pub fn new(range: R) -> Self {
        Self {
            range,
            magnitude: 0.5,
            intercept: 0.0,
        }
    }
}
impl<R> Curve<R> for Cosine<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    #[inline]
    fn range(&self) -> R {
        self.range.clone()
    }

    #[inline]
    fn transform(&self, input: f64) -> f64 {
        let input = self.normalize(input);
        1.0 - (input * std::f64::consts::PI * self.magnitude).cos() + self.intercept
    }
}

#[derive(Debug)]
pub struct Logistic<R = RangeInclusive<f64>> {
    pub range: R,
    pub steepness: f64,
    pub midpoint: f64,
}
impl Default for Logistic {
    fn default() -> Self {
        Self {
            range: 0.0..=1.0,
            steepness: 1.0,
            midpoint: 0.0,
        }
    }
}
impl<R> Logistic<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    pub fn new(range: R) -> Self {
        Self {
            range,
            steepness: 1.0,
            midpoint: 0.0,
        }
    }
}
impl<R> Curve<R> for Logistic<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    #[inline]
    fn range(&self) -> R {
        self.range.clone()
    }

    #[inline]
    fn transform(&self, input: f64) -> f64 {
        let input = self.normalize(input);

        let p = -self.steepness
            * ((4.0 * std::f64::consts::E) * (input - self.midpoint) - 2.0 * std::f64::consts::E);
        1.0 / (1.0 + std::f64::consts::E.powf(p))
    }
}

#[derive(Debug)]
pub struct Logit<R = RangeInclusive<f64>> {
    pub range: R,
    pub base: f64,
}
impl Default for Logit {
    fn default() -> Self {
        Self {
            range: 0.0..=1.0,
            base: 2.72,
        }
    }
}
impl<R> Logit<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    pub fn new(range: R) -> Self {
        Self { range, base: 2.72 }
    }
}
impl<R> Curve<R> for Logit<R>
where
    R: RangeBounds<f64> + Debug + Send + Sync + Clone,
{
    #[inline]
    fn range(&self) -> R {
        self.range.clone()
    }

    #[inline]
    fn transform(&self, input: f64) -> f64 {
        let input = self.normalize(input);

        let t = (input / (1.0 - input)).log(self.base) + 2.0 * std::f64::consts::E;
        t / (4.0 * std::f64::consts::E)
    }
}
