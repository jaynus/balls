#![allow(
    clippy::must_use_candidate,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::use_self
)]
use crossbeam::queue::{ArrayQueue, PushError};
use derivative::Derivative;
use fxhash::FxHashMap;
use std::any::TypeId;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ListenerId(usize);

/// This queue performs per-listener queueing using a crossbeam `ArrayQueue`, pre-defined to an
/// upper limit of messages allowed.
#[derive(Derivative)]
#[derivative(Debug(bound = "T: std::fmt::Debug"))]

pub struct Channel<T> {
    queues: Vec<ArrayQueue<T>>,

    #[derivative(Debug = "ignore")]
    bound_functions: Vec<Box<dyn Fn(T) -> Option<T> + Send + Sync>>,
}

impl<T: Clone> Channel<T> {
    pub fn bind_listener(&mut self, message_capacity: usize) -> ListenerId {
        let new_id = self.queues.len();
        self.queues.push(ArrayQueue::new(message_capacity));

        ListenerId(new_id)
    }

    pub fn bind_exec(&mut self, f: Box<dyn Fn(T) -> Option<T> + Send + Sync>) {
        self.bound_functions.push(f);
    }

    pub fn read(&self, listener_id: ListenerId) -> Option<T> {
        self.queues[listener_id.0].pop().ok()
    }

    pub fn write_iter(&self, iter: impl Iterator<Item = T>) -> Result<(), PushError<T>>
    where
        T: Sync + Send,
    {
        for event in iter {
            self.write(event)?;
        }

        Ok(())
    }

    pub fn write(&self, event: T) -> Result<(), PushError<T>>
    where
        T: Sync + Send,
    {
        if !self
            .bound_functions
            .iter()
            .map(|f| (f)(event.clone()))
            .any(|e| e.is_none())
        {
            self.queues
                .iter()
                .for_each(|queue| queue.push(event.clone()).unwrap());
        }

        Ok(())
    }
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self {
            queues: Vec::new(),
            bound_functions: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EnumChannelListenerId<T> {
    variant: T,
    index: usize,
}

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct EnumChannel<T: Copy + Eq + std::hash::Hash, E: Copy> {
    queues: FxHashMap<T, Vec<ArrayQueue<E>>>,
}

impl<T: Copy + Eq + std::hash::Hash, E: Copy> EnumChannel<T, E> {
    pub fn bind_listener(
        &mut self,
        variant: T,
        message_capacity: usize,
    ) -> EnumChannelListenerId<T> {
        let list = self.queues.entry(variant).or_insert_with(Vec::default);
        let index = list.len();
        list.push(ArrayQueue::new(message_capacity));

        EnumChannelListenerId { variant, index }
    }

    pub fn read(&self, listener_id: EnumChannelListenerId<T>) -> Option<E> {
        self.queues
            .get(&listener_id.variant)?
            .get(listener_id.index)
            .unwrap()
            .pop()
            .ok()
    }

    pub fn write(&self, variant: T, event: E) -> Result<(), PushError<E>> {
        if let Some(queues) = self.queues.get(&variant) {
            for queue in queues {
                queue.push(event)?;
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct TypeChannel<E: Copy> {
    queues: FxHashMap<TypeId, ArrayQueue<E>>,
}

impl<E: Copy> TypeChannel<E> {
    pub fn read<T: 'static>(&self) -> Option<E> {
        self.queues.get(&TypeId::of::<T>())?.pop().ok()
    }

    pub fn write<T: 'static>(&self, args: E) -> Result<(), PushError<E>> {
        self.queues.get(&TypeId::of::<T>()).unwrap().push(args)
    }
}

impl<E: Copy> TypeChannel<E> {
    pub fn with_capacity(types: &[TypeId], capacity: usize) -> Self {
        use std::iter::FromIterator;

        let queues = FxHashMap::from_iter(
            types
                .iter()
                .map(|variant| (*variant, ArrayQueue::new(capacity))),
        );

        Self { queues }
    }
}
