use crate::defs::{
    material::MaterialRef, DefinitionDetails, DefinitionLoader, DefinitionResolver,
    DefinitionStorage,
};
use crate::{
    bitflags::*, bitflags_serial, data::PartGraphId, fxhash::FxHashMap, legion::prelude::*,
    petgraph, ron,
};
use rl_macros::Definition;
use std::{path::Path, sync::Arc};

bitflags_serial! {
    pub struct PartFlag: u32 {
        const STANCE        = 1 << 1;
        const MANIPULATE    = 1 << 2;
        const ORGAN         = 1 << 3;
        const LIMB          = 1 << 4;

        const SKELETON      = 1 << 5;
        const NERVOUS       = 1 << 6;
        const THOUGHT       = 1 << 7;
        const FLIGHT        = 1 << 8;
        const HEAR          = 1 << 9;
        const SIGHT         = 1 << 10;
        const SMELL         = 1 << 11;
        const EATING        = 1 << 12;

        const CIRCULATION   = 1 << 13;
        const RESPITORY     = 1 << 14;

    }
}

bitflags_serial! {
    pub struct PartRelation: u32 {
        const INSIDE    = 1 << 1;
        const OUTSIDE   = 1 << 2;
        const CONNECTED = 1 << 3;
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum TissueKind {
    Bone,
    Fat,
    Nervous,
    Sinew,
    Skin,
    Scale,
    Chitin,
    Fur,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PartLayerKind {
    Tissue {
        kind: TissueKind,
        hardness: u32,
        warmth: u32,
    },
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PartLayer {
    pub material: MaterialRef,
    pub kind: PartLayerKind,
    pub thickness: u32,
}

#[derive(Definition, Debug, serde::Deserialize, serde::Serialize)]
pub struct PartDefinition {
    pub details: DefinitionDetails,
    #[serde(skip)]
    pub id: PartDefinitionId,

    pub group: Option<String>,
    pub category: Option<String>,

    pub relative_size: u32,
    pub flags: PartFlag,
    pub layers: Vec<PartLayer>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PartConnection {
    from: PartRef,
    to: Option<PartRef>,
    relation: PartRelation,
}

#[derive(Definition, Debug, serde::Deserialize, serde::Serialize)]
pub struct PartGroupDefinition {
    pub details: DefinitionDetails,

    #[serde(skip)]
    pub id: PartGroupDefinitionId,

    pub parts: Vec<PartConnection>,
}

impl Into<(String, PartGroupRef)> for PartGroupRef {
    fn into(self) -> (String, Self) {
        (self.name.clone(), self)
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PartGroupConnectionLink {
    pub group: String,
    pub part: PartRef,
}
impl PartGroupConnectionLink {
    pub fn new<S: AsRef<str>>(group: S, part: PartRef) -> Self {
        Self {
            group: group.as_ref().to_string(),
            part,
        }
    }

    pub fn resolve(
        &mut self,
        parts: &DefinitionStorage<PartDefinition>,
    ) -> Result<(), anyhow::Error> {
        self.part.resolve(parts)
    }
}
impl From<(String, PartRef)> for PartGroupConnectionLink {
    fn from((group, part): (String, PartRef)) -> Self {
        Self::new(group, part)
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PartGroupConnection {
    pub relation: PartRelation,
    pub left: PartGroupConnectionLink,
    pub right: PartGroupConnectionLink,
}
impl PartGroupConnection {
    pub fn new(left: (String, PartRef), right: (String, PartRef), relation: PartRelation) -> Self {
        Self {
            left: left.into(),
            right: right.into(),
            relation,
        }
    }
    pub fn resolve(
        &mut self,
        parts: &DefinitionStorage<PartDefinition>,
    ) -> Result<(), anyhow::Error> {
        self.left.resolve(parts)?;
        self.right.resolve(parts)
    }
}

#[derive(Definition, Debug, serde::Deserialize, serde::Serialize)]
#[definition(
    loader = "BodyDefinitionLoader",
    resolver = "Self",
    component = "BodyComponent"
)]
pub struct BodyDefinition {
    pub details: DefinitionDetails,
    #[serde(skip)]
    pub id: BodyDefinitionId,

    pub groups: Vec<(String, PartGroupRef)>,
    pub connections: Vec<PartGroupConnection>,

    #[serde(skip)]
    pub graph: petgraph::graph::UnGraph<Arc<PartDefinition>, PartRelation>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct PartState;

#[derive(Debug, Default, Clone)]
pub struct BodyComponent {
    def: BodyDefinitionId,
    pub flags: PartFlag,
    pub part_states: FxHashMap<PartGraphId, PartState>,
}
impl BodyComponent {
    pub fn new(
        def: BodyDefinitionId,
        storage: &crate::defs::DefinitionStorage<BodyDefinition>,
    ) -> Self {
        let body = def.fetch(storage);
        let mut flags = PartFlag::empty();

        let part_states = body
            .graph
            .node_indices()
            .map(|idx| {
                flags |= body.graph.node_weight(idx).unwrap().flags;
                (idx, PartState::default())
            })
            .collect();

        Self {
            def,
            part_states,
            flags,
        }
    }
}
impl crate::defs::DefinitionComponent<BodyDefinition> for BodyComponent {
    fn id(&self) -> BodyDefinitionId {
        self.def
    }

    fn fetch<'a>(
        &self,
        storage: &'a crate::defs::DefinitionStorage<BodyDefinition>,
    ) -> &'a BodyDefinition {
        self.def.fetch(storage)
    }
}

impl DefinitionResolver<Self> for PartGroupDefinition {
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        let parts = resources
            .get::<DefinitionStorage<PartDefinition>>()
            .unwrap();

        for connection in &mut def.parts {
            connection.from.resolve(&parts)?;
            if let Some(to) = connection.to.as_mut() {
                to.resolve(&parts)?;
            }
        }

        Ok(())
    }
}

impl DefinitionResolver<Self> for BodyDefinition {
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        let groups = resources
            .get::<DefinitionStorage<PartGroupDefinition>>()
            .unwrap();

        let parts = resources
            .get::<DefinitionStorage<PartDefinition>>()
            .unwrap();

        for connection in &mut def.connections {
            connection.resolve(&parts)?;
        }

        for (_, group) in &mut def.groups {
            group.resolve(&groups)?;
        }

        // TODO: count parts and groups
        let mut graph = petgraph::graph::UnGraph::default();

        // Resolve the graph and populate it
        // TODO: better error messages
        let mut nodes = FxHashMap::default();

        for (group_name, group) in &def.groups {
            let group = group.fetch(&groups).expect("Invalid group name");

            group.parts.iter().for_each(|connection| {
                let from_key = format!("{}:{}", group_name, connection.from.name);
                if !nodes.contains_key(&from_key) {
                    nodes.insert(
                        from_key.clone(),
                        graph.add_node(connection.from.clone_ptr()),
                    );
                }

                if let Some(to) = &connection.to {
                    let to_key = format!("{}:{}", group_name, to.name);
                    if !nodes.contains_key(&to_key) {
                        nodes.insert(to_key.clone(), graph.add_node(to.clone_ptr()));
                    }
                    graph.add_edge(nodes[&from_key], nodes[&to_key], connection.relation);
                }
            });
        }

        // Walk through the connections, and link the group nodes
        for connection in &def.connections {
            let left_key = format!("{}:{}", connection.left.group, connection.left.part.name);
            let right_key = format!("{}:{}", connection.right.group, connection.right.part.name);

            graph.add_edge(nodes[&left_key], nodes[&right_key], connection.relation);
        }

        def.graph = graph;

        Ok(())
    }
}

pub struct BodyDefinitionLoader;

mod loader {
    use super::*;

    #[derive(serde::Deserialize, serde::Serialize)]
    pub enum BodyDefProxy {
        Body(BodyDefinition),
        Part(PartDefinition),
        PartGroup(PartGroupDefinition),
    }

    impl DefinitionLoader for BodyDefinitionLoader {
        fn from_folder<P>(resources: &mut Resources, folder: P) -> Result<(), anyhow::Error>
        where
            P: AsRef<Path>,
        {
            let mut bodies = DefinitionStorage::<BodyDefinition>::default();
            let mut parts = DefinitionStorage::<PartDefinition>::default();
            let mut groups = DefinitionStorage::<PartGroupDefinition>::default();

            let files = Self::collect_files(folder);

            for entry in files {
                let contents = std::fs::read_to_string(entry)?;
                let def_entries = ron::de::from_str::<Vec<BodyDefProxy>>(&contents)?;

                for def in def_entries {
                    match def {
                        BodyDefProxy::Body(def) => bodies.insert(def),
                        BodyDefProxy::Part(def) => parts.insert(def),
                        BodyDefProxy::PartGroup(def) => groups.insert(def),
                    }
                }
            }

            resources.insert(parts);

            groups.iter_mut().for_each(|def| {
                PartGroupDefinition::resolve(Arc::get_mut(def).unwrap(), &resources).unwrap()
            });
            resources.insert(groups);

            bodies.iter_mut().for_each(|def| {
                BodyDefinition::resolve(Arc::get_mut(def).unwrap(), &resources).unwrap()
            });
            resources.insert(bodies);

            Ok(())
        }

        fn reload(resources: &mut Resources) -> Result<(), anyhow::Error> {
            let source = resources
                .remove::<DefinitionStorage<BodyDefinition>>()
                .unwrap()
                .source;

            Self::from_folder(resources, source)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{loader::BodyDefProxy, *};
    use crate::GameState;
    use petgraph::dot::Dot;

    #[test]
    fn gen_body_graph() -> Result<(), anyhow::Error> {
        let mut state = GameState::default();

        DefinitionStorage::<BodyDefinition>::from_folder(
            &mut state.resources,
            "../assets/defs/bodies",
        )?;
        let bodies = state
            .resources
            .get::<DefinitionStorage<BodyDefinition>>()
            .unwrap();
        let body = bodies.get_by_name("Human").unwrap();

        println!("{:?}", body);
        println!("-----------------------------");

        println!(
            "{:?}",
            Dot::with_config(
                &body
                    .graph
                    .map(|_, node| node.details.name.clone(), |_, edge| *edge,),
                &[]
            )
        );
        Ok(())
    }

    #[test]
    fn create_human_body() -> Result<(), anyhow::Error> {
        let mut storage = Vec::<BodyDefProxy>::default();

        storage.push(BodyDefProxy::Part(PartDefinition {
            id: PartDefinitionId(0),
            details: DefinitionDetails::new("Head"),
            category: Some("Human".to_string()),
            group: Some("Head".to_string()),
            relative_size: 100,
            flags: PartFlag::NERVOUS | PartFlag::THOUGHT,
            layers: vec![],
        }));
        storage.push(BodyDefProxy::Part(PartDefinition {
            id: PartDefinitionId(0),
            details: DefinitionDetails::new("Neck"),
            category: Some("Human".to_string()),
            group: Some("Head".to_string()),
            relative_size: 100,
            flags: PartFlag::NERVOUS | PartFlag::CIRCULATION,
            layers: vec![],
        }));

        storage.push(BodyDefProxy::Part(PartDefinition {
            id: PartDefinitionId(0),
            details: DefinitionDetails::new("Torso"),
            category: Some("Human".to_string()),
            group: Some("Head".to_string()),
            relative_size: 100,
            flags: PartFlag::CIRCULATION,
            layers: vec![],
        }));
        storage.push(BodyDefProxy::PartGroup(PartGroupDefinition {
            details: DefinitionDetails::new("Head"),
            id: PartGroupDefinitionId(0),
            parts: vec![PartConnection {
                from: PartRef::from("Head"),
                to: Some(PartRef::from("Neck")),
                relation: PartRelation::OUTSIDE | PartRelation::CONNECTED,
            }],
        }));
        storage.push(BodyDefProxy::PartGroup(PartGroupDefinition {
            details: DefinitionDetails::new("UpperBody"),
            id: PartGroupDefinitionId(0),
            parts: vec![PartConnection {
                from: PartRef::from("Torso"),
                to: None,
                relation: PartRelation::OUTSIDE | PartRelation::CONNECTED,
            }],
        }));
        storage.push(BodyDefProxy::Body(BodyDefinition {
            details: DefinitionDetails::new("Human"),
            id: BodyDefinitionId(0),
            groups: vec![
                PartGroupRef::new("Head").into(),
                PartGroupRef::new("UpperBody").into(),
            ],
            connections: vec![PartGroupConnection::new(
                ("Head".into(), "Neck".into()),
                ("UpperBody".into(), "Torso".into()),
                PartRelation::OUTSIDE | PartRelation::CONNECTED,
            )],
            graph: Default::default(),
        }));

        let s = ron::ser::to_string_pretty(
            &storage,
            ron::ser::PrettyConfig::default()
                .with_depth_limit(3)
                .with_extensions(
                    ron::extensions::Extensions::UNWRAP_NEWTYPES
                        | ron::extensions::Extensions::IMPLICIT_SOME,
                ),
        )?;

        println!("{}", s);

        let _ = ron::de::from_str::<Vec<BodyDefProxy>>(&s)?;

        Ok(())
    }
}
