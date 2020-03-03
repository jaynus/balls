#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    dead_code,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    non_camel_case_types
)]
pub mod body;
pub mod building;
pub mod common;
pub mod condition;
pub mod creature;
pub mod foliage;
pub mod item;
pub mod material;
pub mod needs;
pub mod race;
pub mod reaction;
pub mod workshop;

use crate::{
    bit_set::BitSet, derivative::Derivative, failure, fxhash::FxHashMap, legion::prelude::*, ron,
    serde,
};
use std::{
    convert::TryInto,
    path::{Path, PathBuf},
    sync::Arc,
};
use walkdir::WalkDir;

pub trait DefinitionId:
    From<usize> + Into<usize> + Copy + Ord + Eq + std::hash::Hash + std::fmt::Debug
{
    type Definition: Definition;

    fn as_str<'a>(&self, storage: &'a DefinitionStorage<Self::Definition>) -> &'a str;
}

pub trait Definition: std::fmt::Debug
where
    Self: Sized,
{
    type Id: DefinitionId;
    type Component;
    type Loader: DefinitionLoader;
    type Resolver: DefinitionResolver<Self>;

    fn details(&self) -> &DefinitionDetails;

    fn name(&self) -> &str;
    fn display_name(&self) -> &str;
    fn description(&self) -> &str;
    fn long_description(&self) -> &str;

    fn id(&self) -> Self::Id;
    fn set_id(&mut self, _id: Self::Id);

    //fn component(&self) -> Self::Component;
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DefinitionDetails {
    name: String,
    #[serde(default)]
    display_name: String,
    description: String,
    #[serde(default)]
    long_description: String,
}
impl DefinitionDetails {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            display_name: name.to_owned(),
            description: String::default(),
            long_description: String::default(),
        }
    }
}

pub trait DefinitionResolver<T> {
    fn resolve(def: &mut T, resources: &Resources) -> Result<(), failure::Error>;
}

pub trait DefinitionComponent<T: Definition> {
    fn id(&self) -> T::Id;

    fn fetch<'a>(&self, storage: &'a crate::defs::DefinitionStorage<T>) -> &'a T;
}

pub struct DefaultDefinitionResolver<T>(std::marker::PhantomData<T>);
impl<T> DefinitionResolver<T> for DefaultDefinitionResolver<T> {
    fn resolve(_def: &mut T, _resources: &Resources) -> Result<(), failure::Error> {
        Ok(())
    }
}

pub trait DefinitionLoader {
    fn from_folder<P: AsRef<Path>>(
        resources: &mut Resources,
        folder: P,
    ) -> Result<(), failure::Error>;

    fn reload(resources: &mut Resources) -> Result<(), failure::Error>;

    fn collect_files<P: AsRef<Path>>(folder: P) -> Vec<PathBuf> {
        let mut files = Vec::new();

        // Add the root entry
        let root_file = folder.as_ref().with_extension("def");
        if let Ok(meta) = std::fs::metadata(&root_file) {
            if meta.file_type().is_file() {
                files.push(root_file);
            }
        }

        for entry in WalkDir::new(folder.as_ref())
            .into_iter()
            .filter_map(Result::ok)
        {
            if let Ok(meta) = entry.metadata() {
                if meta.file_type().is_file() {
                    files.push(entry.into_path());
                }
            }
        }

        files.sort();
        files
    }
}

pub struct DefaultDefinitionLoader<T: Definition> {
    _marker: std::marker::PhantomData<T>,
}
impl<T> DefinitionLoader for DefaultDefinitionLoader<T>
where
    T: 'static + std::fmt::Debug + Definition + for<'a> serde::Deserialize<'a> + Send + Sync,
{
    fn from_folder<P>(resources: &mut Resources, folder: P) -> Result<(), failure::Error>
    where
        P: AsRef<Path>,
    {
        let mut storage = DefinitionStorage::<T>::default();

        // Collect the files and try the root name as well.
        let files = Self::collect_files(folder);

        for entry in files {
            let contents = std::fs::read_to_string(&entry)
                .map_err(|e| failure::format_err!("[{}]: {}", entry.as_path().display(), e))?;
            let def_entries = ron::de::from_str::<Vec<T>>(&contents)
                .map_err(|e| failure::format_err!("[{}]: {}", entry.as_path().display(), e))?;

            storage
                .storage
                .reserve(storage.storage.len() + def_entries.len());
            storage
                .lookup
                .reserve(storage.lookup.len() + def_entries.len());

            for def in def_entries {
                storage.insert(def);
            }
        }

        storage.resolve(resources)?;

        resources.insert(storage);

        Ok(())
    }

    fn reload(resources: &mut Resources) -> Result<(), failure::Error> {
        let source = resources.remove::<DefinitionStorage<T>>().unwrap().source;

        Self::from_folder(resources, source)
    }
}

#[derive(Derivative, Debug)]
#[derivative(Default(bound = ""))]
pub struct DefinitionStorage<T: Definition> {
    bitset: BitSet,
    lookup: FxHashMap<String, usize>,
    storage: Vec<Arc<T>>,
    source: PathBuf,
}

impl<T> DefinitionStorage<T>
where
    T: std::fmt::Debug + Definition + for<'a> serde::Deserialize<'a>,
{
    pub fn find(&self, name: &str) -> Option<&T> {
        self.get(self.get_id(name)?)
    }

    pub fn get(&self, id: T::Id) -> Option<&T> {
        self.storage.get(id.into()).map(std::convert::AsRef::as_ref)
    }

    pub(crate) fn get_raw(&self, id: T::Id) -> Option<&Arc<T>> {
        self.storage.get(id.into())
    }

    pub fn get_id(&self, name: &str) -> Option<T::Id> {
        self.lookup.get(&name.to_lowercase()).map(|id| (*id).into())
    }

    pub fn get_by_name(&self, name: &str) -> Option<&T> {
        self.get(self.get_id(name)?)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.lookup.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<T>> + '_ {
        self.storage.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Arc<T>> + '_ {
        self.storage.iter_mut()
    }

    pub fn has_key(&self, name: &str) -> bool {
        self.lookup.get(name).is_some()
    }

    pub fn has_id(&self, id: T::Id) -> bool {
        self.storage.len() >= id.into()
    }

    pub fn bitset(&self) -> &BitSet {
        &self.bitset
    }

    pub fn len(&self) -> usize {
        self.storage.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() < 1
    }

    pub fn resolve(&mut self, resources: &mut Resources) -> Result<(), failure::Error> {
        for def in self.iter_mut() {
            T::Resolver::resolve(
                Arc::get_mut(def)
                    .ok_or_else(|| failure::err_msg("Failed to get mutable Arc for definition"))?,
                resources,
            )?;
        }
        Ok(())
    }

    pub fn reload(resources: &mut Resources) -> Result<(), failure::Error> {
        T::Loader::reload(resources)
    }

    pub fn from_folder<P>(resources: &mut Resources, folder: P) -> Result<(), failure::Error>
    where
        P: AsRef<Path>,
    {
        T::Loader::from_folder(resources, folder)
    }

    pub fn insert(&mut self, mut def: T) {
        def.set_id(self.storage.len().into());
        self.bitset.insert(def.id().into());

        self.lookup
            .insert(def.name().to_lowercase(), def.id().try_into().unwrap());

        self.storage.push(Arc::new(def));
    }

    pub fn new<P>(folder: &P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            source: folder.as_ref().to_path_buf(),
            bitset: BitSet::default(),
            lookup: FxHashMap::default(),
            storage: Vec::default(),
        }
    }
}

pub fn load_all_defs(resources: &mut Resources, root: Option<&str>) -> Result<(), failure::Error> {
    let root = Path::new(root.unwrap_or_default());

    DefinitionStorage::<material::MaterialDefinition>::from_folder(
        resources,
        root.join("assets/defs/materials"),
    )?;

    DefinitionStorage::<item::ItemDefinition>::from_folder(
        resources,
        root.join("assets/defs/items"),
    )?;

    DefinitionStorage::<reaction::ReactionDefinition>::from_folder(
        resources,
        root.join("assets/defs/reactions"),
    )?;

    ////
    DefinitionStorage::<foliage::FoliageDefinition>::from_folder(
        resources,
        root.join("assets/defs/foliages"),
    )?;

    ////

    DefinitionStorage::<body::BodyDefinition>::from_folder(
        resources,
        root.join("assets/defs/bodies"),
    )?;

    DefinitionStorage::<race::RaceDefinition>::from_folder(
        resources,
        root.join("assets/defs/races"),
    )?;

    DefinitionStorage::<creature::CreatureDefinition>::from_folder(
        resources,
        root.join("assets/defs/creatures"),
    )?;

    ////

    DefinitionStorage::<building::BuildingDefinition>::from_folder(
        resources,
        root.join("assets/defs/buildings"),
    )?;

    DefinitionStorage::<workshop::WorkshopDefinition>::from_folder(
        resources,
        root.join("assets/defs/workshops"),
    )?;

    // assert storage lengths
    assert!(
        resources
            .get::<DefinitionStorage<reaction::ReactionDefinition>>()
            .unwrap()
            .len()
            > 0
    );
    assert!(
        resources
            .get::<DefinitionStorage<workshop::WorkshopDefinition>>()
            .unwrap()
            .len()
            > 0
    );
    assert!(
        resources
            .get::<DefinitionStorage<material::MaterialDefinition>>()
            .unwrap()
            .len()
            > 0
    );
    assert!(
        resources
            .get::<DefinitionStorage<item::ItemDefinition>>()
            .unwrap()
            .len()
            > 0
    );

    assert!(
        resources
            .get::<DefinitionStorage<body::BodyDefinition>>()
            .unwrap()
            .len()
            > 0
    );

    Ok(())
}
