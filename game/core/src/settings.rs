use rl_render_pod::color::Color;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DisplayMode {
    Fullscreen,
    Windowed(u32, u32),
}
impl Default for DisplayMode {
    fn default() -> Self {
        Self::Windowed(1280, 1024)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub window_title: String,
    pub display_mode: DisplayMode,
    palette: Palette,

    pub version: u64,
}
impl Settings {
    pub fn palette(&self) -> &Palette {
        &self.palette
    }
    pub fn palette_mut(&mut self) -> &mut Palette {
        self.version += 1;
        &mut self.palette
    }
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            window_title: "balls".to_string(),
            display_mode: DisplayMode::default(),
            palette: Palette::default(),
            version: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Palette {
    // Base colors
    pub red: Color,
    pub blue: Color,
    pub green: Color,
    pub black: Color,
    pub brown: Color,

    pub blue_empty: Color,
    pub bright_green: Color,

    pub pawn: Color,
    pub task_designation: Color,
    pub stockpile: Color,
    pub placement: Color,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            red: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            blue: Color {
                r: 63.0 / 255.0,
                g: 127.0 / 255.0,
                b: 191.0 / 255.0,
                a: 1.0,
            },

            black: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            brown: Color {
                r: 150.0 / 255.0,
                g: 75.0 / 255.0,
                b: 0.0,
                a: 1.0,
            },
            green: Color {
                r: 0.0,
                g: 99.0 / 255.0,
                b: 16.0 / 255.0,
                a: 1.0,
            },

            blue_empty: Color {
                r: 63.0 / 255.0,
                g: 127.0 / 255.0,
                b: 191.0 / 255.0,
                a: 50.0 / 255.0,
            },

            bright_green: Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },

            pawn: Color {
                r: 63.0 / 255.0,
                g: 127.0 / 255.0,
                b: 191.0 / 255.0,
                a: 1.0,
            },

            // specifics
            task_designation: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },

            stockpile: Color {
                r: 63.0 / 255.0,
                g: 127.0 / 255.0,
                b: 191. / 255.0,
                a: 1.0,
            },

            placement: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }
    }
}
