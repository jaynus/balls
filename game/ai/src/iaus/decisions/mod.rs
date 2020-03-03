use crate::iaus::{Consideration, DecisionSet};
use rl_core::{failure, legion::prelude::*, slotmap, NamedSlotMap};
use std::sync::Arc;

slotmap::new_key_type! { pub struct DecisionHandle; }

pub type DecisionStorage = NamedSlotMap<DecisionHandle, DecisionSet>;

pub struct DecisionSetRegistration {
    name: String,
    inner: Vec<Arc<dyn Consideration>>,
}
impl DecisionSetRegistration {
    pub fn new(name: &str, inner: Vec<Arc<dyn Consideration>>) -> Self {
        Self {
            name: name.to_string(),
            inner,
        }
    }
}

pub fn decision_registration() -> Vec<DecisionSetRegistration> {
    vec![
        DecisionSetRegistration::new("idle", vec![considerations::idle()]),
        DecisionSetRegistration::new("work", vec![considerations::work()]),
        DecisionSetRegistration::new("thirst", vec![considerations::thirst()]),
        DecisionSetRegistration::new("hunger", vec![considerations::hunger()]),
    ]
}

pub fn prepare(_: &mut World, resources: &mut Resources) -> Result<(), failure::Error> {
    let mut decisions = DecisionStorage::default();

    for consideration in decision_registration() {
        decisions.insert(
            &consideration.name,
            DecisionSet::new(&consideration.name, consideration.inner.clone()),
        );
    }

    resources.insert(decisions);

    Ok(())
}

pub mod considerations {
    use super::*;
    use crate::iaus::*;
    use rl_core::defs::needs::NeedKind;
    use std::sync::Arc;

    pub fn idle() -> Arc<dyn Consideration> {
        Arc::new(ClosureConsideration::new("idle", Box::new(|_| 0.3)))
    }

    pub fn work() -> Arc<dyn Consideration> {
        Arc::new(CurveConsideration::new(
            "work",
            Some(Box::new(curves::Linear {
                range: std::ops::Range {
                    start: f64::from(i16::min_value()),
                    end: f64::from(i16::max_value()),
                },
                slope: 1.0,
                intercept: 0.0,
            })),
            Box::new(|consideration, _| consideration.curve.as_ref().unwrap().transform(30.0)),
        ))
    }

    pub fn hunger() -> Arc<dyn Consideration> {
        Arc::new(CurveConsideration::new(
            "hunger",
            Some(Box::new(curves::Linear {
                range: std::ops::Range {
                    start: -500.0,
                    end: 500.0,
                },
                slope: 1.0,
                intercept: -0.4,
            })),
            Box::new(|consideration, state| {
                consideration
                    .curve
                    .as_ref()
                    .unwrap()
                    .transform(1.0 - f64::from(state.needs.get(NeedKind::Calories).value))
            }),
        ))
    }

    pub fn thirst() -> Arc<dyn Consideration> {
        Arc::new(CurveConsideration::new(
            "thirst",
            Some(Box::new(curves::Linear {
                range: std::ops::Range {
                    start: -500.0,
                    end: 500.0,
                },
                slope: 1.0,
                intercept: -0.4,
            })),
            Box::new(|consideration, state| {
                consideration
                    .curve
                    .as_ref()
                    .unwrap()
                    .transform(1.0 - f64::from(state.needs.get(NeedKind::Hydration).value))
            }),
        ))
    }
}
