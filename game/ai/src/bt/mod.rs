use crate::SensesComponent;
use rl_core::{
    components::{BlackboardComponent, DeadTag, MovementComponent},
    derivative::Derivative,
    legion::prelude::*,
    shrinkwrap::Shrinkwrap,
    slotmap,
    strum_macros::{AsRefStr, EnumDiscriminants},
    GameStateRef, NamedSlotMap,
};
use std::sync::Arc;

slotmap::new_key_type! { pub struct BehaviorHandle; }

#[derive(Shrinkwrap, Default, Clone)]
#[shrinkwrap(mutable)]
pub struct BehaviorStorage(pub NamedSlotMap<BehaviorHandle, Arc<dyn BehaviorNode>>);

#[derive(Debug, Copy, Clone, PartialEq, EnumDiscriminants, AsRefStr)]
#[strum_discriminants(name(BehaviorRootKind))]
pub enum BehaviorRoot {
    Decision(BehaviorHandle),
    Forced(BehaviorHandle),
    None,
}
impl Default for BehaviorRoot {
    fn default() -> Self {
        Self::None
    }
}
impl BehaviorRoot {
    pub fn is_none(&self) -> bool {
        BehaviorRootKind::None == (*self).into()
    }

    pub fn is_forced(&self) -> bool {
        BehaviorRootKind::Forced == (*self).into()
    }

    pub fn handle(&self) -> Option<BehaviorHandle> {
        match *self {
            Self::Decision(handle) | Self::Forced(handle) => Some(handle),
            Self::None => None,
        }
    }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct BehaviorTreeComponent {
    pub root: BehaviorRoot,
    pub last_root: BehaviorRoot,
    pub last_status: BehaviorStatus,
    pub last_frame: u64,
    //#[derivative(Debug = "ignore")]
    //pub status_cache: Vec<(Arc<dyn BehaviorNode>, bool)>,
}
impl Default for BehaviorTreeComponent {
    fn default() -> Self {
        Self {
            root: BehaviorRoot::None,
            last_root: BehaviorRoot::None,
            last_status: BehaviorStatus::success(),
            last_frame: 0,
            //status_cache: Vec::default(),
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct BehaviorArgs<'a> {
    pub entity: Entity,
    #[derivative(Debug = "ignore")]
    pub blackboard: &'a mut BlackboardComponent,
    pub tree: &'a BehaviorTreeComponent,
    pub senses: &'a SensesComponent,
    #[derivative(Debug = "ignore")]
    pub command_buffer: &'a mut CommandBuffer,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BehaviorError {
    Todo, // TODO:
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BehaviorResult {
    Running,
    Success,
    Failure,
    Error(BehaviorError),
}
impl Default for BehaviorResult {
    fn default() -> Self {
        Self::Running
    }
}

#[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub struct BehaviorStatus {
    result: BehaviorResult,
    cancellable: bool,
    bail: bool,
}
impl BehaviorStatus {
    pub fn running(cancellable: bool) -> Self {
        Self {
            result: BehaviorResult::Running,
            cancellable,
            bail: false,
        }
    }

    pub fn bail() -> Self {
        Self {
            result: BehaviorResult::Failure,
            cancellable: false,
            bail: true,
        }
    }

    pub fn failure() -> Self {
        Self {
            result: BehaviorResult::Failure,
            cancellable: true,
            bail: false,
        }
    }

    pub fn success() -> Self {
        Self {
            result: BehaviorResult::Success,
            cancellable: true,
            bail: false,
        }
    }

    pub fn is_running(self) -> bool {
        self.result == BehaviorResult::Running
    }

    pub fn is_complete(self) -> bool {
        self.result == BehaviorResult::Success || self.result == BehaviorResult::Failure
    }

    pub fn is_cancellable(self) -> bool {
        if self.is_running() {
            self.cancellable
        } else {
            true
        }
    }
}
impl PartialEq for BehaviorStatus {
    fn eq(&self, rhv: &Self) -> bool {
        self.result.eq(&rhv.result)
    }
}
impl PartialEq<BehaviorResult> for BehaviorStatus {
    fn eq(&self, rhv: &BehaviorResult) -> bool {
        self.result.eq(&rhv)
    }
}

pub trait BehaviorNode: Send + Sync {
    fn eval(&self, _state: GameStateRef, _args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        unimplemented!()
    }
}

pub fn system(world: &mut World, _: &mut Resources) -> Box<dyn FnMut(&mut World, &mut Resources)> {
    game_metrics::scope!("behavior_execution_system");

    let pawn_query = <(
        Read<SensesComponent>,
        Write<BehaviorTreeComponent>,
        Write<BlackboardComponent>,
    )>::query()
    .filter(!tag::<DeadTag>());

    let mut command_buffer = CommandBuffer::new(world);

    Box::new(move |world: &mut World, resources: &mut Resources| {
        let behavior_storage = <Read<BehaviorStorage>>::fetch(&resources);

        let entities = unsafe {
            pawn_query
                .iter_entities_unchecked(world)
                .map(|(entity, (_, _, _))| entity)
                .collect::<Vec<_>>()
        };

        for entity in &entities {
            {
                let senses = world.get_component::<SensesComponent>(*entity).unwrap();
                let mut blackboard =
                    unsafe { world.get_component_mut_unchecked::<BlackboardComponent>(*entity) }
                        .unwrap();
                let mut tree =
                    unsafe { world.get_component_mut_unchecked::<BehaviorTreeComponent>(*entity) }
                        .unwrap();

                if let Some(root_handle) = tree.root.handle() {
                    if tree.root == tree.last_root {
                        let status = behavior_storage.get(root_handle).unwrap().eval(
                            GameStateRef { world, resources },
                            &mut BehaviorArgs {
                                entity: *entity,
                                blackboard: &mut blackboard,
                                senses: &senses,
                                tree: &tree,
                                command_buffer: &mut command_buffer,
                            },
                        );

                        tree.last_status = status;
                        if status.is_complete() {
                            tree.root = BehaviorRoot::None;
                            blackboard.clear();
                        }
                        if status.bail {
                            tree.root = BehaviorRoot::None;
                            blackboard.clear();

                            // Clear any movement requests
                            unsafe {
                                // TODO: cleaner action cancellation for bails.
                                world
                                    .get_component_mut_unchecked::<MovementComponent>(*entity)
                                    .unwrap()
                                    .current = None;
                            }
                        }
                    } else {
                        // Utility system executes the behavior once on run, so we need to skip it
                        tree.last_root = tree.root;
                    }
                }
            }
            command_buffer.write(world);
        }
    })
}

impl<F> BehaviorNode for (Option<String>, F)
where
    F: Fn(GameStateRef, &mut BehaviorArgs<'_>) -> BehaviorStatus + Send + Sync,
{
    fn eval(&self, state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        if let Some(condition) = self.0.as_ref() {
            if !validate_conditions(state, args, condition.as_str()) {
                return BehaviorStatus::failure();
            }
        }

        (self.1)(state, args)
    }
}

#[derive(AsRefStr)]
pub enum BehaviorKind {
    Selector(Vec<Arc<dyn BehaviorNode>>),
    Sequence(Vec<Arc<dyn BehaviorNode>>),
    ReverseResult(Arc<dyn BehaviorNode>),
    All(Vec<Arc<dyn BehaviorNode>>),
    ForLoop {
        limit: usize,
        condition: BehaviorResult,
        node: Arc<dyn BehaviorNode>,
    },
}
impl std::fmt::Debug for BehaviorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl BehaviorNode for BehaviorKind {
    fn eval(&self, state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        match self {
            Self::ReverseResult(child) => {
                let mut res = child.eval(state, args);
                match res.result {
                    BehaviorResult::Running => {}
                    BehaviorResult::Success => res.result = BehaviorResult::Failure,
                    BehaviorResult::Failure => res.result = BehaviorResult::Success,
                    BehaviorResult::Error(e) => panic!(e),
                };

                res
            }
            Self::Selector(children) => {
                for child in children {
                    let res = child.eval(state, args);
                    if res.bail {
                        return res;
                    }

                    match res.result {
                        BehaviorResult::Running => {
                            return res;
                        }
                        BehaviorResult::Success => return BehaviorStatus::success(),
                        BehaviorResult::Failure => {}
                        BehaviorResult::Error(e) => panic!(e),
                    };
                }

                // If we get here, this branch is fails, because nothing was selected
                BehaviorStatus::failure()
            }
            Self::Sequence(children) => {
                for child in children {
                    let res = child.eval(state, args);
                    if res.bail {
                        return res;
                    }

                    match res.result {
                        BehaviorResult::Running => {
                            return res;
                        }
                        BehaviorResult::Success => {}
                        BehaviorResult::Failure => return BehaviorStatus::failure(),
                        BehaviorResult::Error(e) => panic!(e),
                    };
                }

                // If we get here, this branch is complete
                BehaviorStatus::success()
            }
            Self::All(children) => {
                for child in children {
                    let res = child.eval(state, args);
                    if res.bail {
                        return res;
                    }
                    match res.result {
                        BehaviorResult::Running => {
                            return res;
                        }
                        BehaviorResult::Success | BehaviorResult::Failure => {}
                        BehaviorResult::Error(e) => panic!(e),
                    };
                }

                // If we get here, this branch is complete
                BehaviorStatus::success()
            }
            Self::ForLoop {
                limit,
                condition,
                node,
            } => {
                let mut n = 0;
                let mut result = node.eval(state, args);
                if result.bail {
                    return result;
                }

                while result != *condition {
                    result = node.eval(state, args);
                    if result.bail {
                        return result;
                    }
                    n += 1;
                    if n >= *limit {
                        break;
                    }
                }
                result
            }
        }
    }
}

pub mod make {
    use super::*;

    pub fn not(inner: Arc<dyn BehaviorNode>) -> Arc<dyn BehaviorNode> {
        Arc::new(BehaviorKind::ReverseResult(inner))
    }

    pub fn selector(inner: &[Arc<dyn BehaviorNode>]) -> Arc<dyn BehaviorNode> {
        Arc::new(BehaviorKind::Selector(inner.to_vec()))
    }

    pub fn sequence(inner: &[Arc<dyn BehaviorNode>]) -> Arc<dyn BehaviorNode> {
        Arc::new(BehaviorKind::Sequence(inner.to_vec()))
    }

    pub fn all(inner: &[Arc<dyn BehaviorNode>]) -> Arc<dyn BehaviorNode> {
        Arc::new(BehaviorKind::All(inner.to_vec()))
    }

    pub fn try_until(
        limit: usize,
        condition: BehaviorResult,
        node: Arc<dyn BehaviorNode>,
    ) -> Arc<dyn BehaviorNode> {
        Arc::new(BehaviorKind::ForLoop {
            limit,
            condition,
            node,
        })
    }

    pub fn if_else(
        condition: Arc<dyn BehaviorNode>,
        success: Arc<dyn BehaviorNode>,
        failure: Arc<dyn BehaviorNode>,
    ) -> Arc<dyn BehaviorNode> {
        selector(&[sequence(&[condition, success]), failure])
    }

    pub fn switch(name: &str, storage: &BehaviorStorage) -> Arc<dyn BehaviorNode> {
        let handle = storage.get_handle(name).unwrap();
        closure(None, move |state, args| {
            unsafe {
                state
                    .world
                    .get_component_mut_unchecked::<BehaviorTreeComponent>(args.entity)
            }
            .unwrap()
            .root = BehaviorRoot::Forced(handle);
            unsafe {
                state
                    .world
                    .get_component_mut_unchecked::<BlackboardComponent>(args.entity)
            }
            .unwrap()
            .clear();

            BehaviorStatus::success()
        })
    }

    pub fn sub(name: &str, storage: &BehaviorStorage) -> Arc<dyn BehaviorNode> {
        storage.get_by_name(name).unwrap().clone()
    }

    pub fn closure<F>(condition: Option<&str>, f: F) -> Arc<dyn BehaviorNode>
    where
        F: 'static + Fn(GameStateRef, &mut BehaviorArgs<'_>) -> BehaviorStatus + Send + Sync,
    {
        Arc::new((condition.map(std::string::ToString::to_string), f))
    }
}

pub fn validate_conditions(
    _state: GameStateRef,
    _args: &mut BehaviorArgs<'_>,
    _condition: &str,
) -> bool {
    true
}
