use crate::{fnv_hash, FnvHasher};
use downcast_rs::{impl_downcast, DowncastSync};
use shrinkwraprs::Shrinkwrap;
use std::{collections::HashMap, hash::BuildHasherDefault};

pub trait BlackboardValue: 'static + DowncastSync {}
impl<T> BlackboardValue for T where T: 'static + Send + Sync {}
impl_downcast!(BlackboardValue);

pub type BlackboardComponent = Blackboard;

#[derive(Default, Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct Blackboard(pub HashMap<u64, Box<dyn BlackboardValue>, BuildHasherDefault<FnvHasher>>);
impl Blackboard {
    #[inline]
    pub fn remove_by_name(&mut self, name: &str) {
        self.remove(fnv_hash(name));
    }

    #[inline]
    pub fn remove(&mut self, hash: u64) {
        self.0.remove(&hash);
    }

    #[inline]
    pub fn remove_get<T: BlackboardValue>(&mut self, hash: u64) -> Option<T> {
        Some(*self.0.remove(&hash)?.downcast::<T>().ok()?)
    }

    #[inline]

    pub fn contains(&self, hash: u64) -> bool {
        self.0.contains_key(&hash)
    }

    #[inline]
    pub fn contains_name(&mut self, name: &str) -> bool {
        self.0.contains_key(&fnv_hash(name))
    }

    #[inline]
    pub fn insert_by_name<T: BlackboardValue>(&mut self, name: &str, value: T) -> Option<T> {
        self.insert(fnv_hash(name), value)
    }
    #[inline]
    pub fn insert<T: BlackboardValue>(&mut self, hash: u64, value: T) -> Option<T> {
        Some(*self.0.insert(hash, Box::new(value))?.downcast::<T>().ok()?)
    }

    #[inline]

    pub fn get<T: BlackboardValue>(&self, hash: u64) -> Option<&T> {
        self.0.get(&hash).map(|v| v.downcast_ref::<T>().unwrap())
    }

    #[inline]
    pub fn get_mut<T: BlackboardValue>(&mut self, hash: u64) -> Option<&mut T> {
        self.0
            .get_mut(&hash)
            .map(|v| v.downcast_mut::<T>().unwrap())
    }

    #[inline]

    pub fn get_by_name<T: BlackboardValue>(&self, name: &str) -> Option<&T> {
        self.get(fnv_hash(name))
    }

    #[inline]
    pub fn get_by_name_mut<T: BlackboardValue>(&mut self, name: &str) -> Option<&mut T> {
        self.get_mut(fnv_hash(name))
    }

    #[inline]

    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }
}
