pub mod entity {
    use legion::prelude::*;
    use serde::ser::SerializeSeq;
    use smallvec::SmallVec;
    use std::{cell::RefCell, collections::HashMap, sync::Arc};
    use uuid::Uuid;

    pub trait EntityList<'a> {
        fn entities_as_slice(&self) -> &[Entity];
        fn entities_as_mut_slice(&mut self) -> &mut [Entity];

        fn entities_push(&mut self, entity: Entity);

        fn entities_extend(&mut self, iter: impl Iterator<Item = Entity>);
    }

    impl<'a, T> EntityList<'a> for SmallVec<T>
    where
        T: smallvec::Array<Item = Entity>,
    {
        fn entities_as_slice(&self) -> &[Entity] {
            self.as_slice()
        }
        fn entities_as_mut_slice(&mut self) -> &mut [Entity] {
            self.as_mut_slice()
        }
        fn entities_push(&mut self, entity: Entity) {
            self.push(entity);
        }
        fn entities_extend(&mut self, iter: impl Iterator<Item = Entity>) {
            self.extend(iter)
        }
    }

    impl<'a> EntityList<'a> for Vec<Entity> {
        fn entities_as_slice(&self) -> &[Entity] {
            self.as_slice()
        }
        fn entities_as_mut_slice(&mut self) -> &mut [Entity] {
            self.as_mut_slice()
        }
        fn entities_push(&mut self, entity: Entity) {
            self.push(entity);
        }
        fn entities_extend(&mut self, iter: impl Iterator<Item = Entity>) {
            self.extend(iter)
        }
    }

    thread_local! {
        pub static SER_ENTITY_MAP: RefCell<Arc<RefCell<HashMap<Entity, type_uuid::Bytes>>>> = RefCell::new(Arc::new(RefCell::new(HashMap::new())));
        pub static DE_ENTITY_MAP: RefCell<Arc<RefCell<HashMap<type_uuid::Bytes, Entity>>>> = RefCell::new(Arc::new(RefCell::new(HashMap::new())));
    }

    #[allow(clippy::trivially_copy_pass_by_ref)] // serde required
    pub fn serialize<S>(entity: &Entity, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&SER_ENTITY_MAP.with(|map| {
            *map.borrow_mut()
                .borrow_mut()
                .entry(*entity)
                .or_insert(*Uuid::new_v4().as_bytes())
        }))
    }

    pub fn deserialize<'e, 'de, D>(deserializer: D) -> Result<Entity, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct UuidBytesVisitor;

        impl<'de> serde::de::Visitor<'de> for UuidBytesVisitor {
            type Value = Entity;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a byte representation of a uuid")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let uuid = Uuid::from_slice(v).map_err(E::custom)?;
                DE_ENTITY_MAP.with(|map| {
                    map.borrow_mut()
                        .borrow_mut()
                        .get(uuid.as_bytes())
                        .copied()
                        .ok_or_else(|| E::custom("Entity doesnt exist"))
                })
            }
        }

        deserializer.deserialize_bytes(UuidBytesVisitor)
    }

    pub mod list {
        use super::{EntityList, SerializeSeq, Uuid, SER_ENTITY_MAP};

        pub fn serialize<'e, T, S>(entities: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            T: EntityList<'e>,
            S: serde::Serializer,
        {
            let slice = entities.entities_as_slice();

            let mut seq = serializer.serialize_seq(Some(slice.len()))?;
            for entity in slice.iter() {
                let bytes = &SER_ENTITY_MAP.with(|map| {
                    *map.borrow_mut()
                        .borrow_mut()
                        .entry(*entity)
                        .or_insert(*Uuid::new_v4().as_bytes())
                });

                seq.serialize_element(bytes)?;
            }
            seq.end()
        }

        #[allow(clippy::needless_pass_by_value)] // serde sig requires
        pub fn deserialize<'e, 'de, T, D>(_deserializer: D) -> Result<T, D::Error>
        where
            T: EntityList<'e>,
            D: serde::Deserializer<'de>,
        {
            unimplemented!()
        }
    }
}
