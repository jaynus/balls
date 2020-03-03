use rl_ai::iaus::*;
use rl_core::legion::prelude::*;
use rl_ui::{
    imgui::{self, im_str, ImString},
    UiWindowSet,
};
use std::collections::HashMap;

pub trait CurveEditor: Send + Sync {
    fn draw(&mut self, ui: &imgui::Ui);
    fn transform(&self, input: f64) -> f64;
}

mod editors {
    use super::*;

    #[derive(Default)]
    pub struct Linear {
        curve: curves::Linear,
    }
    impl CurveEditor for Linear {
        fn draw(&mut self, ui: &imgui::Ui) {
            imgui::Slider::new(im_str!("slope"), 0.0..=7.0).build(ui, &mut self.curve.slope);
            imgui::Slider::new(im_str!("intercept"), -1.0..=1.0)
                .build(ui, &mut self.curve.intercept);
        }
        fn transform(&self, input: f64) -> f64 {
            self.curve.transform(input)
        }
    }

    pub struct Exponential {
        curve: curves::Exponential,
    }
    impl Default for Exponential {
        fn default() -> Self {
            Self {
                curve: curves::Exponential {
                    range: 0.0..=1.0,
                    power: 2.0,
                    intercept: 0.0,
                },
            }
        }
    }
    impl CurveEditor for Exponential {
        fn draw(&mut self, ui: &imgui::Ui) {
            imgui::Slider::new(im_str!("power"), 0.0..=20.0).build(ui, &mut self.curve.power);
            imgui::Slider::new(im_str!("intercept"), -1.0..=1.0)
                .build(ui, &mut self.curve.intercept);
        }
        fn transform(&self, input: f64) -> f64 {
            self.curve.transform(input)
        }
    }

    #[derive(Default)]
    pub struct Sine {
        curve: curves::Sine,
    }
    impl CurveEditor for Sine {
        fn draw(&mut self, ui: &imgui::Ui) {
            imgui::Slider::new(im_str!("magnitude"), 0.0..=1.0)
                .build(ui, &mut self.curve.magnitude);
            imgui::Slider::new(im_str!("intercept"), -1.0..=1.0)
                .build(ui, &mut self.curve.intercept);
        }
        fn transform(&self, input: f64) -> f64 {
            self.curve.transform(input)
        }
    }

    #[derive(Default)]
    pub struct Cosine {
        curve: curves::Cosine,
    }
    impl CurveEditor for Cosine {
        fn draw(&mut self, ui: &imgui::Ui) {
            imgui::Slider::new(im_str!("magnitude"), 0.0..=1.0)
                .build(ui, &mut self.curve.magnitude);
            imgui::Slider::new(im_str!("intercept"), -1.0..=1.0)
                .build(ui, &mut self.curve.intercept);
        }
        fn transform(&self, input: f64) -> f64 {
            self.curve.transform(input)
        }
    }

    #[derive(Default)]
    pub struct Logistic {
        curve: curves::Logistic,
    }
    impl CurveEditor for Logistic {
        fn draw(&mut self, ui: &imgui::Ui) {
            imgui::Slider::new(im_str!("steepness"), -1.0..=1.0)
                .build(ui, &mut self.curve.steepness);
            imgui::Slider::new(im_str!("midpoint"), -1.0..=1.0).build(ui, &mut self.curve.midpoint);
        }
        fn transform(&self, input: f64) -> f64 {
            self.curve.transform(input)
        }
    }

    #[derive(Default)]
    pub struct Logit {
        curve: curves::Logit,
    }
    impl CurveEditor for Logit {
        fn draw(&mut self, ui: &imgui::Ui) {
            imgui::Slider::new(im_str!("base"), 0.0..=5.0).build(ui, &mut self.curve.base);
        }
        fn transform(&self, input: f64) -> f64 {
            self.curve.transform(input)
        }
    }
}

#[allow(clippy::cast_precision_loss)]
pub fn build(world: &mut World, resources: &Resources) {
    #[derive(Default)]
    struct EditorState {
        decision_idx: usize,
        consideration_idx: usize,
        curve_idx: usize,

        decision_names: Vec<String>,
        im_decision_names: Vec<ImString>,

        consideration_names: Vec<String>,
        im_consideration_names: Vec<ImString>,
    }

    let mut state = EditorState::default();

    let mut curves: HashMap<String, Box<dyn CurveEditor>> = Default::default();
    curves.insert("Linear".to_string(), Box::new(editors::Linear::default()));
    curves.insert(
        "Exponential".to_string(),
        Box::new(editors::Exponential::default()),
    );
    curves.insert("Sine".to_string(), Box::new(editors::Sine::default()));
    curves.insert("Cosine".to_string(), Box::new(editors::Cosine::default()));
    curves.insert(
        "Logistic".to_string(),
        Box::new(editors::Logistic::default()),
    );
    curves.insert("Logit".to_string(), Box::new(editors::Logit::default()));

    let im_curves = curves
        .keys()
        .map(|name| ImString::from(name.clone()))
        .collect::<Vec<_>>();

    let test_values = (0..100).map(|n| f64::from(n) * 0.01).collect::<Vec<_>>();

    #[allow(unused_mut)]
    let mut decisions = resources.get::<decisions::DecisionStorage>().unwrap();

    state.decision_names = decisions
        .iter()
        .map(|(name, _, _)| name.to_string())
        .collect::<Vec<_>>();

    state.im_decision_names = state
        .decision_names
        .iter()
        .map(|name| ImString::from(name.clone()))
        .collect::<Vec<_>>();

    state.consideration_names = decisions
        .get_by_name(&state.decision_names[0])
        .unwrap()
        .considerations()
        .iter()
        .map(|c| c.name().to_string())
        .collect::<Vec<_>>();

    state.im_consideration_names = state
        .consideration_names
        .iter()
        .map(|name| ImString::from(name.clone()))
        .collect::<Vec<_>>();

    UiWindowSet::create_with(
        world,
        resources,
        "iaus_editor",
        false,
        move |ui, _, _, resources, _| {
            let mut decisions =
                unsafe { <Write<decisions::DecisionStorage>>::fetch_unchecked(&resources) };

            imgui::Window::new(im_str!("iaus_editor"))
                .position([0.0, 0.0], imgui::Condition::FirstUseEver)
                .size([550.0, 550.0], imgui::Condition::FirstUseEver)
                .build(ui, || {
                    if imgui::ComboBox::new(im_str!("Select Decision")).build_simple_string(
                        ui,
                        &mut state.decision_idx,
                        state
                            .im_decision_names
                            .iter()
                            .collect::<Vec<_>>()
                            .as_slice(),
                    ) {
                        let cur_decision = decisions
                            .get_by_name(&state.decision_names[state.decision_idx])
                            .unwrap();

                        state.consideration_idx = 0;
                        state.consideration_names = cur_decision
                            .considerations()
                            .iter()
                            .map(|c| c.name().to_string())
                            .collect::<Vec<_>>();

                        state.im_consideration_names = state
                            .consideration_names
                            .iter()
                            .map(|name| ImString::from(name.clone()))
                            .collect::<Vec<_>>();

                        // Pick its curve
                    }

                    let _ = decisions
                        .get_by_name_mut(&state.decision_names[state.decision_idx])
                        .unwrap();

                    imgui::ComboBox::new(im_str!("Consideration")).build_simple_string(
                        ui,
                        &mut state.consideration_idx,
                        state
                            .im_consideration_names
                            .iter()
                            .collect::<Vec<_>>()
                            .as_slice(),
                    );

                    imgui::ComboBox::new(im_str!("Curve")).build_simple_string(
                        ui,
                        &mut state.curve_idx,
                        im_curves.iter().collect::<Vec<_>>().as_slice(),
                    );

                    // Test create a curve and output it here
                    let values = test_values
                        .iter()
                        .map(|n| curves[&im_curves[state.curve_idx].to_string()].transform(*n))
                        .map(|n| n as f32)
                        .collect::<Vec<_>>();

                    imgui::PlotLines::new(ui, &im_curves[state.curve_idx], &values)
                        .scale_min(0.0)
                        .scale_max(1.0)
                        .graph_size([400.0, 400.0])
                        .build();

                    curves
                        .get_mut(&im_curves[state.curve_idx].to_string())
                        .unwrap()
                        .draw(ui);
                });

            true
        },
    );
}
