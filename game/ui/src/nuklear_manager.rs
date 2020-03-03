use rl_core::{
    app, failure,
    winit::{self},
    GameState, Manager,
};

pub struct NuklearManager {
    nuklear: nuklear::Context,
    font_config: nuklear::FontConfig,
    font_atlas: nuklear::FontAtlas,
}
impl NuklearManager {
    pub fn new(_state: &mut GameState) -> Result<Self, failure::Error> {
        /*
        let mut cfg = nuklear::FontConfig::with_size(0.0);
        cfg.set_oversample_h(3);
        cfg.set_oversample_v(2);
        cfg.set_glyph_range(nuklear::font_cyrillic_glyph_ranges());
        cfg.set_ttf(include_bytes!("../../assets/ui/fonts/Roboto-Regular.ttf"));

        let mut allo = nuklear::Allocator::new_vec();

        let mut atlas = nuklear::FontAtlas::new(&mut allo);

        cfg.set_ttf_data_owned_by_atlas(false);
        cfg.set_size(14f32);
        let font_14 = atlas.add_font_with_config(&cfg).unwrap();

        cfg.set_ttf_data_owned_by_atlas(false);
        cfg.set_size(18f32);
        let font_18 = atlas.add_font_with_config(&cfg).unwrap();

        cfg.set_ttf_data_owned_by_atlas(false);
        cfg.set_size(20f32);
        let font_20 = atlas.add_font_with_config(&cfg).unwrap();

        cfg.set_ttf_data_owned_by_atlas(false);
        cfg.set_size(22f32);
        let font_22 = atlas.add_font_with_config(&cfg).unwrap();

        let font_tex = {
            let (b, w, h) = atlas.bake(nuklear::FontAtlasFormat::Rgba32);
            // TODO:: ADD FONT TEXTURE HERE????????
        };

        let mut null = nuklear::DrawNullTexture::default();

        atlas.end(font_tex, Some(&mut null));
        //atlas.cleanup();

        let mut ctx = nuklear::Context::new(&mut allo, atlas.font(font_14).unwrap().handle());
        */
        unimplemented!()
    }
}
impl Manager for NuklearManager {
    fn tick(
        &mut self,
        _context: &mut app::ApplicationContext,
        _state: &mut GameState,
    ) -> Result<(), failure::Error> {
        Ok(())
    }

    fn on_event<'a>(
        &mut self,
        _context: &mut app::ApplicationContext,
        _state: &mut GameState,
        event: &'a winit::event::Event<()>,
    ) -> Result<Option<&'a winit::event::Event<'a, ()>>, failure::Error> {
        Ok(Some(event))
    }
}
