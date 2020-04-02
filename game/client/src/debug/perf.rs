use rl_core::{legion::prelude::*, metrics::Metrics, Logging};
use rl_ui::{
    imgui::{self, im_str},
    UiWindowSet,
};

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
pub fn build(world: &mut World, resources: &mut Resources) -> Result<(), anyhow::Error> {
    struct PerfWindowState {
        open: bool,
    }

    resources.insert::<PerfWindowState>(PerfWindowState { open: true });

    UiWindowSet::create_with(
        world,
        resources,
        "perf_viewer",
        false,
        move |ui, _, _, resources, _| {
            let (_logging, metrics, mut window_state) = unsafe {
                <(Read<Logging>, Read<Metrics>, Write<PerfWindowState>)>::fetch_unchecked(resources)
            };
            if window_state.open {
                imgui::Window::new(im_str!("Performance##UI"))
                    .position([0.0, 0.0], imgui::Condition::FirstUseEver)
                    .size([300.0, 200.0], imgui::Condition::FirstUseEver)
                    .opened(&mut window_state.open)
                    .build(ui, || {
                        ui.columns(4, im_str!(""), true);
                        ui.set_column_width(0, 200.0);
                        ui.text("Name");
                        ui.next_column();
                        ui.text("mean");
                        ui.next_column();
                        ui.text("min");
                        ui.next_column();
                        ui.text("max");
                        ui.next_column();

                        metrics.for_each_histogram(|span_name, h| {
                            ui.text(span_name);
                            ui.next_column();
                            ui.text(&format!("{:.2}", h.mean() / 1_000_000.0));
                            ui.next_column();
                            ui.text(&format!("{:.2}", h.min() as f64 / 1_000_000.0));
                            ui.next_column();
                            ui.text(&format!("{:.2}", h.max() as f64 / 1_000_000.0));
                            ui.next_column();
                        });
                    });
            }

            let visible = window_state.open;
            window_state.open = true;
            visible
        },
    );

    Ok(())
}
