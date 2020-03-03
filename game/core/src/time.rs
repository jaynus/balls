use arraydeque::{ArrayDeque, Wrapping};
use chrono::prelude::*;
use parking_lot::RwLock;
use slotmap::SlotMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GameDate {
    start_date: DateTime<Utc>,
    current_date: DateTime<Utc>,
}
impl GameDate {
    pub fn new(start_date: DateTime<Utc>) -> Self {
        Self {
            start_date,
            current_date: start_date,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TimeStamp {
    pub real: f64,
    pub world: f64,
    pub frame: u64,
}

#[derive(Debug)]
pub struct Time {
    pub world_delta: Duration,
    pub real_delta: Duration,

    pub world_speed: f32,
    pub real_speed: f32,

    pub world_time: f64,
    pub real_time: f64,

    pub last_tick: Instant,

    pub fps_accumluator: ArrayDeque<[f32; 32], Wrapping>,
    pub current_fps: f32,

    pub frame: u64,

    pub timers: RwLock<SlotMap<TimerHandle, Timer>>,
}
impl Default for Time {
    fn default() -> Self {
        Self {
            world_delta: Duration::new(0, 0),
            real_delta: Duration::new(0, 0),
            world_time: 0.0,
            real_time: 0.0,
            world_speed: 0.0,
            real_speed: 1.0,
            last_tick: Instant::now(),
            fps_accumluator: Default::default(),
            current_fps: 0.0,
            frame: 0,
            timers: RwLock::new(Default::default()),
        }
    }
}

impl Time {
    #[allow(clippy::cast_precision_loss)]
    pub fn tick(&mut self) {
        let now = Instant::now();
        let duration = now - self.last_tick;
        self.last_tick = now;

        self.real_delta = duration.mul_f32(self.real_speed);
        self.world_delta = duration.mul_f32(self.world_speed);

        self.real_time += self.real_delta.as_secs_f64();
        // Dont real time update, tick world time update
        //self.world_time += self.world_delta.as_secs_f64();

        self.fps_accumluator
            .push_back(self.real_delta.as_secs_f32());

        self.current_fps =
            1.0 / (self.fps_accumluator.iter().sum::<f32>() / self.fps_accumluator.len() as f32);

        // Clear any completed timers
        let mut timers = self.timers.write();

        let remove = timers
            .iter()
            .filter_map(|(key, timer)| {
                if timer.is_complete(self.real_time, self.world_time) {
                    Some(key)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.frame += 1;

        remove.iter().for_each(|key| {
            timers.remove(*key).unwrap();
        });
    }

    pub fn create_world_timer(&self, duration: Duration) -> TimerHandle {
        self.timers.write().insert(Timer::new(
            self.world_time + duration.as_secs_f64(),
            duration,
        ))
    }

    /// Returns true for non-existant timers and complete timers
    pub fn is_timer_complete(&self, handle: TimerHandle) -> bool {
        let res = self.timers.read().get(handle).map_or(true, |timer| {
            timer.is_complete(self.real_time, self.world_time)
        });

        // If it was complete, remove it
        if res {
            self.timers.write().remove(handle);
        }

        res
    }

    pub fn timer_progress(&self, handle: TimerHandle) -> f64 {
        self.timers
            .read()
            .get(handle)
            .unwrap()
            .progress(self.real_time, self.world_time)
    }

    pub fn stamp(&self) -> TimeStamp {
        TimeStamp {
            world: self.world_time,
            real: self.real_time,
            frame: self.frame,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum TimerKind {
    World,
    Real,
}
impl Default for TimerKind {
    fn default() -> Self {
        Self::World
    }
}

slotmap::new_key_type! { pub struct TimerHandle; }

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Timer {
    pub kind: TimerKind,
    pub end: f64,
    pub duration: Duration,
}
impl Timer {
    fn new(end: f64, duration: Duration) -> Self {
        Self {
            kind: TimerKind::default(),
            end,
            duration,
        }
    }

    #[inline]
    fn progress(&self, real_time: f64, world_time: f64) -> f64 {
        if self.is_complete(real_time, world_time) {
            return 1.0;
        }

        match self.kind {
            TimerKind::World => 1.0 - (self.end - world_time / self.duration.as_secs_f64()),
            TimerKind::Real => 1.0 - (self.end - real_time / self.duration.as_secs_f64()),
        }
    }

    #[inline]
    fn is_complete(&self, real_time: f64, world_time: f64) -> bool {
        match self.kind {
            TimerKind::World => world_time >= self.end,
            TimerKind::Real => real_time >= self.end,
        }
    }
}
