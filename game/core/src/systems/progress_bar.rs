use crate::{
    legion::prelude::*,
    math::{Lerp, Vec2, Vec3},
    settings::Settings,
    time::{Time, TimerHandle},
};
use rl_render_pod::sprite::{SparseSprite, SparseSpriteArray, SparseSpriteHandle};
/*

let test_progress_bar = SparseSprite {
    position: Vec3::new(0.0, 0.0, 0.0),
    u_offset: Vec2::new(0.5, 1.0),
    v_offset: Vec2::new(0.0, 0.5),
    color: Color::black(),
    dir_x: Vec2::new(32.0, 0.0),
    dir_y: Vec2::new(0.0, -32.0),
};
SparseSpriteArray::from_slice(&[test_progress_bar]),*/

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct ProgressBar {
    pub progress: f64,
    indices: Option<(SparseSpriteHandle, SparseSpriteHandle)>,
    timer: Option<TimerHandle>,
}
impl ProgressBar {
    pub fn with_timer(timer: TimerHandle) -> Self {
        Self {
            progress: 0.0,
            timer: Some(timer),
            indices: None,
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
pub fn build(_: &mut World, _: &mut Resources) -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("progress_bar_system")
        .read_resource::<Time>()
        .read_resource::<crate::map::Map>()
        .read_resource::<Settings>()
        .with_query(
            <(Write<Option<ProgressBar>>, Write<SparseSpriteArray>)>::query()
                .filter(changed::<Option<ProgressBar>>()),
        )
        .build(move |_, world, (time, _map, settings), bar_query| {
            if time.real_delta.as_secs_f32() > 1.0 {
                // Skip a bad frame
                return;
            }

            let palette = settings.palette();

            for (mut progress_bar, mut sprites) in bar_query.iter_mut(world) {
                if let Some(progress_bar) = progress_bar.as_mut() {
                    // New entry, add its sprites and track them
                    if progress_bar.indices.is_none() {
                        let bar_index = sprites.insert(SparseSprite {
                            position: Vec3::new(0.0, -16.0, 0.002),
                            u_offset: Vec2::new(0.0, 0.5),
                            v_offset: Vec2::new(0.0, 0.5),
                            color: palette.bright_green,
                            dir_x: Vec2::new(28.0, 0.0),
                            dir_y: Vec2::new(0.0, -10.0),
                        });

                        let border_index = sprites.insert(SparseSprite {
                            position: Vec3::new(0.0, -16.0, 0.001),
                            u_offset: Vec2::new(0.5, 1.0),
                            v_offset: Vec2::new(0.0, 0.5),
                            color: palette.black,
                            dir_x: Vec2::new(32.0, 0.0),
                            dir_y: Vec2::new(0.0, -32.0),
                        });
                        progress_bar.indices = Some((border_index, bar_index));
                    }

                    if let Some(timer) = progress_bar.timer {
                        progress_bar.progress = time.timer_progress(timer);
                    }

                    if let Some((border_index, bar_index)) = progress_bar.indices {
                        if progress_bar.progress >= 1.0 {
                            sprites.remove(border_index);
                            sprites.remove(bar_index);
                        } else {
                            // Lerp over 10 second
                            // Min = 0, max = 28

                            sprites[bar_index].dir_x =
                                Vec2::new(0.0.lerp(28.0, progress_bar.progress as f32), 0.0);
                        }
                    }
                }
            }
        })
}
