use crate::{ImguiContextLock, ImguiContextWrapper};
use imgui_winit_support::WinitPlatform;
use rl_core::{
    app,
    input::InputState,
    legion::prelude::*,
    winit::{self},
    GameState, Manager,
};
use std::sync::{Arc, Mutex};

pub struct ImguiManager {
    imgui: ImguiContextLock,
}
impl ImguiManager {
    pub fn new(state: &mut GameState) -> Result<Self, anyhow::Error> {
        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(Some(std::path::Path::new("data/imgui.ini").to_path_buf()));
        let platform = WinitPlatform::init(&mut imgui);

        let imgui = Arc::new(Mutex::new(ImguiContextWrapper::new(imgui)));
        state.resources.insert(imgui.clone());
        state.resources.insert(platform);

        Ok(Self { imgui })
    }

    pub fn imgui_context(&self) -> ImguiContextLock {
        self.imgui.clone()
    }
}
impl Manager for ImguiManager {
    fn tick(
        &mut self,
        _context: &mut app::ApplicationContext,
        state: &mut GameState,
    ) -> Result<(), anyhow::Error> {
        let mut input_state = state.resources.get_mut::<InputState>().unwrap();
        let imgui = &self.imgui.lock().unwrap().context;

        if imgui.io().want_capture_mouse {
            input_state.ignore_mouse = true;
        } else {
            input_state.ignore_mouse = false;
        }

        if imgui.io().want_capture_keyboard {
            input_state.ignore_keyboard = true;
        } else {
            input_state.ignore_keyboard = false;
        }

        Ok(())
    }

    fn on_event<'a>(
        &mut self,
        _context: &mut app::ApplicationContext,
        state: &mut GameState,
        event: &'a winit::event::Event<()>,
    ) -> Result<Option<&'a winit::event::Event<'a, ()>>, anyhow::Error> {
        // Handle events

        let (window, mut platform) =
            <(Read<Arc<winit::window::Window>>, Write<WinitPlatform>)>::fetch_mut(
                &mut state.resources,
            );

        platform.handle_event(
            self.imgui.lock().unwrap().context.io_mut(),
            &**window,
            &event,
        );

        Ok(Some(event))
    }
}
