#![feature(stmt_expr_attributes, vec_remove_item, fn_traits)]
#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    dead_code,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    clippy::expect_fun_call,
    non_camel_case_types
)]

use legion::prelude::*;
use num_traits::{FromPrimitive, ToPrimitive};
use proc_macro_hack::proc_macro_hack;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use ultraviolet::{Vec3, Vec3i};

#[macro_export]
macro_rules! init_array(
    ($len:expr, $val:expr) => (
        {
            #[allow(deprecated)]
            let mut array: [_; $len] = unsafe { std::mem::uninitialized() };
            for i in array.iter_mut() {
                unsafe { ::std::ptr::write(i, $val); }
            }
            array
        }
    )
);

pub use bit_set;
pub use bitflags;
pub use crossbeam;
pub use crossbeam_channel;
pub use derivative;
pub use failure;
pub use fxhash;
pub use game_metrics as metrics;
pub use image;
pub use legion;
pub use log;
pub use num_traits;
pub use parking_lot;
pub use paste;
pub use petgraph;
pub use purple;
pub use rand;
pub use rand_distr;
pub use rand_xorshift;
pub use rayon;
pub use ron;
pub use rstar;
pub use serde;
pub use shrinkwraprs as shrinkwrap;
pub use slice_deque;
pub use slotmap;
pub use smallvec;
pub use strum;
pub use strum_macros;
pub use toml;
pub use type_uuid;
pub use type_uuid::TypeUuid;
pub use uuid;
pub use winit;

pub mod app;
#[macro_use]
pub mod bitflags_serial;
pub mod blackboard;
pub mod camera;
pub mod components;
pub mod condition;
pub mod data;
pub mod debug;
pub mod defs;
pub mod dispatcher;
pub mod ecs_manager;
pub mod event;
pub mod garbage_collector;
pub mod input;
pub mod inventory;
pub mod map;
pub mod math;
pub mod morton;
pub mod saveload;
pub mod settings;
pub mod systems;
pub mod time;
pub mod transform;

pub fn is_game_tick(state: &GameState) -> bool {
    let mut time = state.resources.get_mut::<time::Time>().unwrap();

    // Update world time for a tick
    time.world_time += 30.0;

    true
}

pub trait Manager {
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

    fn destroy(&mut self, _context: &mut app::ApplicationContext, _state: &mut GameState) {}
}

#[derive(Copy, Clone)]
pub struct GameStateRef<'a> {
    pub resources: &'a Resources,
    pub world: &'a World,
}
impl<'a> From<&'a GameStateMutRef<'a>> for GameStateRef<'a> {
    fn from(other: &'a GameStateMutRef<'a>) -> Self {
        Self {
            resources: other.resources,
            world: other.world,
        }
    }
}
pub struct GameStateMutRef<'a> {
    pub resources: &'a mut Resources,
    pub world: &'a mut World,
}

pub struct GameState {
    pub universe: Universe,
    pub resources: Resources,
    pub world: World,
}
impl Default for GameState {
    fn default() -> Self {
        let universe = Universe::new();
        let resources = Resources::default();
        let world = universe.create_world();
        Self {
            universe,
            world,
            resources,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenDimensions {
    pub size: winit::dpi::LogicalSize<f64>,
    pub dpi: f64,
}

#[derive(Debug)]
pub struct Logging;
//pub dispatcher: tracing::Dispatch,
// pub downcaster: tracing::timing::Downcaster<
//    tracing::timing::group::ByName,
//    tracing::timing::group::ByMessage,
// >,

impl Default for Logging {
    fn default() -> Self {
        Self
    }
}
#[derive(Debug)]
pub struct Allocators {
    frame_arena: purple::Arena,
    static_arena: purple::Arena, // Never deallocates or frames
}

#[derive(derivative::Derivative, Clone)]
#[derivative(Default(bound = ""))]
pub struct NamedSlotMap<H, T>
where
    H: slotmap::Key,
    T: slotmap::Slottable,
{
    names: std::collections::HashMap<String, H>,
    data: slotmap::SlotMap<H, T>,
}
impl<H, T> NamedSlotMap<H, T>
where
    H: Copy + PartialEq + slotmap::Key,
    T: slotmap::Slottable,
{
    pub fn insert_unnamed(&mut self, value: T) -> H {
        self.data.insert(value)
    }

    pub fn insert(&mut self, name: &str, value: T) -> H {
        let handle = self.data.insert(value);
        if self
            .names
            .insert(name.to_string().to_lowercase(), handle)
            .is_some()
        {
            panic!("Duplicate name inserted");
        }

        handle
    }

    #[inline]
    pub fn get_handle(&self, name: &str) -> Option<H> {
        Some(*self.names.get(&name.to_lowercase())?)
    }

    #[inline]
    pub fn get_name(&self, handle: H) -> Option<&str> {
        self.names
            .iter()
            .find_map(|(k, v)| if *v == handle { Some(k.as_str()) } else { None })
    }

    #[inline]
    pub fn get(&self, handle: H) -> Option<&T> {
        self.data.get(handle)
    }

    #[inline]
    pub fn get_mut(&mut self, handle: H) -> Option<&mut T> {
        self.data.get_mut(handle)
    }

    #[inline]

    pub fn get_by_name(&self, name: &str) -> Option<&T> {
        self.data.get(self.get_handle(name)?)
    }

    #[inline]
    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut T> {
        self.data.get_mut(self.get_handle(name)?)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &H, &T)> {
        NamedSlotMapIter {
            inner: self,
            name_iter: self.names.iter(),
        }
    }
}

pub struct NamedSlotMapIter<'a, H, T, I>
where
    H: slotmap::Key,
    T: slotmap::Slottable,
{
    inner: &'a NamedSlotMap<H, T>,
    name_iter: I,
}
impl<'a, H, T, I> Iterator for NamedSlotMapIter<'a, H, T, I>
where
    I: Iterator<Item = (&'a String, &'a H)>,
    H: Copy + PartialEq + slotmap::Key,
    T: slotmap::Slottable,
{
    type Item = (&'a str, &'a H, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let next_pair = self.name_iter.next()?;
        Some((
            next_pair.0,
            next_pair.1,
            self.inner.get(*next_pair.1).unwrap(),
        ))
    }
}

#[derive(Clone)]
pub struct AtomicResult<T> {
    inner: Arc<AtomicUsize>,
    _marker: std::marker::PhantomData<T>,
}
impl<T: Default + FromPrimitive + ToPrimitive> Default for AtomicResult<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
impl<T: FromPrimitive + ToPrimitive> AtomicResult<T> {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(state: T) -> Self {
        Self {
            inner: Arc::new(AtomicUsize::new(state.to_usize().unwrap())),
            _marker: Default::default(),
        }
    }

    #[inline]

    pub fn get(&self) -> T {
        T::from_usize(self.inner.load(Ordering::SeqCst)).unwrap()
    }

    #[inline]
    #[allow(clippy::needless_pass_by_value)]
    pub fn set(&self, value: T) {
        self.inner
            .store(value.to_usize().unwrap(), Ordering::SeqCst)
    }
}

#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct GlobalCommandBuffer(pub legion::command::CommandBuffer);
impl GlobalCommandBuffer {
    pub fn new(world: &mut World) -> Self {
        Self(legion::command::CommandBuffer::new(world))
    }
}
pub fn fnv_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in s.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[proc_macro_hack]
pub use rl_macros::fnv;

#[derive(Default)]
pub struct FnvHasher(u64);
impl std::hash::Hasher for FnvHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    #[allow(clippy::cast_ptr_alignment)]
    fn write(&mut self, bytes: &[u8]) {
        assert_eq!(bytes.len(), 8);
        self.0 = 0;
        self.0 = unsafe { *(bytes.as_ptr() as *const u64) }
    }
}

//#[proc_macro_hack]
//pub use rl_macros::instrument_scope;

pub trait Distance {
    fn distance(&self, other: &Self) -> f32;
}

impl Distance for Vec3 {
    fn distance(&self, other: &Self) -> f32 {
        (*self - *other).mag()
    }
}

impl Distance for Vec3i {
    fn distance(&self, other: &Self) -> f32 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32).distance(&Vec3::new(
            other.x as f32,
            other.y as f32,
            other.y as f32,
        ))
    }
}

#[derive(Clone)]
pub struct FreeList<T> {
    free: Vec<T>,
    consumed: fxhash::FxHashSet<T>,
}
impl<T: Distance + Copy + Eq + std::hash::Hash> FreeList<T> {
    pub fn pop_nearest(&mut self, src: &T) -> Option<T> {
        let nearest = self.free.iter().fold(None, |acc: Option<(T, f32)>, free| {
            let distance = free.distance(src);

            if let Some(acc) = acc {
                if distance < acc.1 {
                    Some((*free, distance))
                } else {
                    Some(acc)
                }
            } else {
                Some((*free, distance))
            }
        });

        if let Some(nearest) = nearest {
            let res = self.free.remove_item(&nearest.0);
            if let Some(res) = res {
                self.consumed.insert(res);
            }
            res
        } else {
            None
        }
    }
}
impl<T: Copy + Eq + std::hash::Hash> FreeList<T> {
    pub fn pop(&mut self) -> Option<T> {
        if let Some(v) = self.free.pop() {
            self.consumed.insert(v);
            return Some(v);
        }
        None
    }
    pub fn push(&mut self, value: T) -> Result<(), failure::Error> {
        if !self.consumed.remove(&value) {
            return Err(failure::format_err!("Item returned not already in the consumed list. This belongs to a different FreeList"));
        }
        self.free.push(value);

        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.free.iter().chain(self.consumed.iter())
    }

    pub fn iter_free(&self) -> impl Iterator<Item = &T> + ExactSizeIterator {
        self.free.iter()
    }

    pub fn iter_consumed(&self) -> impl Iterator<Item = &T> + ExactSizeIterator {
        self.consumed.iter()
    }

    pub fn has_free(&self) -> bool {
        !self.free.is_empty()
    }

    pub fn has_consumed(&self) -> bool {
        !self.consumed.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.free.len() + self.consumed.len()
    }

    pub fn consumed_len(&self) -> usize {
        self.consumed.len()
    }

    pub fn free_len(&self) -> usize {
        self.free.len()
    }
}
impl<T: Copy + Eq + std::hash::Hash> std::iter::FromIterator<T> for FreeList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let free = Vec::from_iter(iter);

        Self {
            consumed: fxhash::FxHashSet::with_capacity_and_hasher(
                free.len(),
                fxhash::FxBuildHasher::default(),
            ),
            free,
        }
    }
}
impl<T: std::fmt::Debug + Eq + std::hash::Hash> std::fmt::Debug for FreeList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FreeList ( consumed: {:?}, free: {:?} )",
            self.free, self.consumed
        )
    }
}

pub struct Random {
    rng: parking_lot::Mutex<XorShiftRng>,
}
impl Random {
    pub fn new(seed: &str) -> Self {
        Self {
            rng: parking_lot::Mutex::new(XorShiftRng::from_seed(Self::seed_from_string(seed))),
        }
    }

    pub fn make(&self) -> XorShiftRng {
        let mut seed: [u8; 16] = [0; 16];
        self.rng.lock().fill(&mut seed);
        XorShiftRng::from_seed(seed)
    }

    pub fn seed_from_string(seed: &str) -> [u8; 16] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.input(seed.as_bytes());

        let mut final_seed: [u8; 16] = [0; 16];
        final_seed.copy_from_slice(&hasher.result()[0..16]);
        final_seed
    }
}
