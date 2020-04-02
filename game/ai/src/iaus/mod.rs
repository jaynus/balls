use crate::utility::UtilityState;
use std::{fmt::Debug, hash::Hash, sync::Arc};

pub mod curves;
pub mod decisions;
pub mod simple;
pub use curves::Curve;

pub use simple::*;

pub trait Consideration: Send + Sync {
    fn name(&self) -> &str;
    fn score(&self, state: &UtilityState) -> f64;
}

impl<F> Consideration for F
where
    F: Fn(&UtilityState) -> f64 + Send + Sync,
{
    fn name(&self) -> &str {
        &"NONAME"
    }

    fn score(&self, state: &UtilityState) -> f64 {
        (self)(state)
    }
}

pub trait Decision: Send + Sync {
    fn name(&self) -> &str;

    fn considerations(&self) -> &[Arc<dyn Consideration>];

    fn base(&self) -> f64 {
        1.0
    }

    #[allow(clippy::cast_precision_loss)]
    fn score(&self, state: &UtilityState) -> f64 {
        let scores = self
            .considerations()
            .iter()
            .filter_map(|consider| {
                let score = consider.score(state);
                if score > 0.0 {
                    Some(score.min(1.0).max(0.0))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let base = self.base();
        let modifier = 1.0 - (1.0 / self.considerations().len() as f64);
        scores.iter().fold(base, |mut acc, score| {
            acc *= *score;
            acc + ((1.0 - acc) * modifier * acc)
        })
    }
}

#[derive(Default, Clone)]
pub struct DecisionSet {
    pub name: String,
    pub set: Vec<Arc<dyn Consideration>>,
}
impl DecisionSet {
    pub fn new(name: &str, set: Vec<Arc<dyn Consideration>>) -> Self {
        Self {
            set,
            name: name.to_string(),
        }
    }
}
impl Decision for DecisionSet {
    fn name(&self) -> &str {
        &self.name
    }
    fn considerations(&self) -> &[Arc<dyn Consideration>] {
        self.set.as_slice()
    }
}

pub struct Bucket<P>
where
    P: Debug + Ord + Hash + Send + Sync,
{
    decisions: Vec<Arc<dyn Decision>>,
    priority: P,
}
impl<P> Bucket<P>
where
    P: Debug + Ord + Hash + Send + Sync,
{
    pub fn new(priority: P) -> Self {
        Self {
            priority,
            decisions: Vec::default(),
        }
    }

    pub fn with_decision(mut self, consideration: Arc<dyn Decision>) -> Self {
        self.decisions.push(consideration);

        self
    }
}
impl<P> PartialEq for Bucket<P>
where
    P: Debug + Ord + Hash + Send + Sync,
{
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}
impl<P> Eq for Bucket<P> where P: Debug + Ord + Hash + Send + Sync {}

impl<P> PartialOrd for Bucket<P>
where
    P: Debug + Ord + Hash + Send + Sync,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.priority.partial_cmp(&other.priority)
    }
}

impl<P> Ord for Bucket<P>
where
    P: Debug + Ord + Hash + Send + Sync,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}
impl<P> Hash for Bucket<P>
where
    P: Debug + Ord + Hash + Send + Sync,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.priority.hash(state);
    }
}
