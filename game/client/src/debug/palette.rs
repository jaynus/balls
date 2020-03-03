use rl_core::{failure, legion::prelude::*, settings::Settings};
use rl_ui::{
    clipboard::{self, ClipboardProvider},
    imgui::{self, im_str},
    UiWindowSet,
};

macro_rules! color_picker {
    ($name:ident, $settings:ident, $ui:ident, $clipboard:ident) => {
        let s = imgui::ImString::new(stringify!($name));
        let mut $name: [f32; 4] = $settings.palette().$name.as_slice().try_into().unwrap();
        if imgui::ColorEdit::new(&s, &mut $name)
            .inputs(false)
            .preview(imgui::ColorPreview::HalfAlpha)
            .build($ui)
        {
            $clipboard
                .set_contents(format!(
                    "( {}, {}, {}, {} )",
                    ($name[0] * 255.0) as u8,
                    ($name[1] * 255.0) as u8,
                    ($name[2] * 255.0) as u8,
                    ($name[3] * 255.0) as u8
                ))
                .unwrap();
            $settings.palette_mut().$name = $name.into();
        }
        $ui.next_column();
    };
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
pub fn build(world: &mut World, resources: &mut Resources) -> Result<(), failure::Error> {
    use std::convert::TryInto;

    struct PaletteEditorState {
        open: bool,
    }

    resources.insert(PaletteEditorState { open: true });

    UiWindowSet::create_with(
        world,
        resources,
        "palette_editor",
        false,
        move |ui, _window_manager, _world, resources, _buffer| {
            let (mut window_state, mut settings) = unsafe {
                <(Write<PaletteEditorState>, Write<Settings>)>::fetch_unchecked(resources)
            };

            if window_state.open {
                imgui::Window::new(im_str!("palette_editor##UI"))
                    .position([0.0, 0.0], imgui::Condition::FirstUseEver)
                    .size([500.0, 500.0], imgui::Condition::FirstUseEver)
                    .opened(&mut window_state.open)
                    .build(ui, || {
                        let mut clipboard: clipboard::ClipboardContext =
                            clipboard::ClipboardProvider::new().unwrap();

                        ui.columns(3, im_str!("colorcols##UI"), true);

                        color_picker!(red, settings, ui, clipboard);
                        color_picker!(blue, settings, ui, clipboard);
                        color_picker!(black, settings, ui, clipboard);
                        color_picker!(brown, settings, ui, clipboard);
                        color_picker!(green, settings, ui, clipboard);
                        color_picker!(blue_empty, settings, ui, clipboard);
                        color_picker!(bright_green, settings, ui, clipboard);

                        color_picker!(pawn, settings, ui, clipboard);
                        color_picker!(task_designation, settings, ui, clipboard);
                        color_picker!(stockpile, settings, ui, clipboard);
                        color_picker!(placement, settings, ui, clipboard);

                        ui.columns(1, im_str!(""), false);
                    });
            }

            let visible = window_state.open;
            window_state.open = true;
            visible
        },
    );

    Ok(())
}
