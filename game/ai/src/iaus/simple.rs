use crate::{
    iaus::{Consideration, Curve},
    utility::UtilityState,
};
use derivative::Derivative;
use std::ops::RangeBounds;

#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
pub struct CurveConsideration<R = std::ops::Range<f64>>
where
    R: RangeBounds<f64> + Send + Sync,
{
    pub name: String,
    pub curve: Option<Box<dyn Curve<R>>>,
    #[derivative(Debug = "ignore")]
    pub function: Box<(dyn Fn(&CurveConsideration<R>, &UtilityState) -> f64 + Send + Sync)>,
}
impl<R> CurveConsideration<R>
where
    R: RangeBounds<f64> + Send + Sync,
{
    pub fn new(
        name: &str,
        curve: Option<Box<dyn Curve<R>>>,
        function: Box<(dyn Fn(&CurveConsideration<R>, &UtilityState) -> f64 + Send + Sync)>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            curve,
            function,
        }
    }
}
impl<R> Consideration for CurveConsideration<R>
where
    R: RangeBounds<f64> + Send + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn score(&self, state: &UtilityState) -> f64 {
        (self.function)(self, state)
    }
}

//////
pub struct ClosureConsideration {
    pub name: String,
    pub function: Box<(dyn FnMut(&UtilityState) -> f64 + Send + Sync)>,
}
impl ClosureConsideration {
    pub fn new(name: &str, function: Box<(dyn FnMut(&UtilityState) -> f64 + Send + Sync)>) -> Self {
        Self {
            name: name.to_owned(),
            function,
        }
    }
}
impl Consideration for ClosureConsideration {
    fn name(&self) -> &str {
        &self.name
    }

    fn score(&self, state: &UtilityState) -> f64 {
        unsafe {
            let ptr = &self.function as *const _;
            let mut_ptr = ptr as *mut Box<(dyn FnMut(&UtilityState) -> f64 + Send + Sync)>;
            (*mut_ptr)(state)
        }
    }
}
