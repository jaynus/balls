use crate::{
    defs::{
        common::Property,
        foliage::FoliageKind,
        item::{ItemAbilityKind, ItemProperty},
        material::MaterialLimit,
        needs::NeedKind,
    },
    map::tile::TileKind,
};
use std::str::FromStr;
use strum_macros::AsRefStr;

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, AsRefStr, serde::Serialize, serde::Deserialize,
)]
pub enum Subject {
    Me,
    Target,
    Any,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, AsRefStr, serde::Serialize, serde::Deserialize,
)]
pub enum Operator {
    True,
    False,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRefStr, serde::Serialize, serde::Deserialize)]
pub enum Item {
    Name(String),
    Property(ItemProperty),
    Ability(ItemAbilityKind),
    Material(Material),
    Nutrition(Nutrition),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRefStr, serde::Serialize, serde::Deserialize)]
pub enum Tile {
    Kind(TileKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRefStr, serde::Serialize, serde::Deserialize)]
pub enum Nutrition {
    Kind(Option<NeedKind>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, AsRefStr, serde::Serialize, serde::Deserialize)]
pub enum Value {
    Property(Property),
    Item(Item),
    Tile(Tile),
    Foliage(FoliageKind),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Material {
    pub op: Operator,
    pub limit: MaterialLimit,
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub subject: Subject,
    pub op: Operator,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct ConditionRight {
    pub op: Operator,
    pub set: Box<ConditionSet>,
}

#[derive(Debug, Clone)]
pub struct ConditionSet {
    pub left: Condition,
    pub right: Option<ConditionRight>,
}

fn to_ron_case(value: &str) -> String {
    let mut res = String::with_capacity(value.len());
    let mut cap_state = true;
    for c in value.chars() {
        if cap_state {
            res.push_str(&c.to_uppercase().to_string());
            cap_state = false;
        } else {
            res.push(c);
        }
        match c {
            '(' | ')' => cap_state = true,
            _ => {}
        }
    }
    res
}

#[derive(Debug, Clone)]
pub struct ConditionSetRef {
    string: String,
    condition: Option<Box<ConditionSet>>,
}
impl ConditionSetRef {
    pub fn new<S>(string: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            string: string.as_ref().to_owned(),
            condition: None,
        }
    }
}
impl PartialEq for ConditionSetRef {
    fn eq(&self, other: &Self) -> bool {
        self.string.eq(&other.string)
    }
}
impl Eq for ConditionSetRef {}
impl std::hash::Hash for ConditionSetRef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.string.hash(state);
    }
}

impl ConditionSetRef {
    pub fn parse(&mut self) -> Result<&ConditionSet, anyhow::Error> {
        self.condition = Some(Box::new(self.string.to_condition_set()?));

        Ok(self.condition.as_ref().unwrap())
    }
}
impl std::ops::Deref for ConditionSetRef {
    type Target = ConditionSet;

    fn deref(&self) -> &Self::Target {
        self.condition.as_ref().unwrap()
    }
}
impl AsRef<ConditionSet> for ConditionSetRef {
    fn as_ref(&self) -> &ConditionSet {
        self.condition.as_ref().unwrap()
    }
}
impl<S> From<S> for ConditionSetRef
where
    S: AsRef<str>,
{
    fn from(string: S) -> Self {
        Self::new(string)
    }
}

impl serde::Serialize for ConditionSetRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.string)
    }
}
impl<'de> serde::Deserialize<'de> for ConditionSetRef {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ConditionSetRef, D::Error> {
        deserializer.deserialize_str(ConditionSetRefVisitor)
    }
}
struct ConditionSetRefVisitor;
impl<'de> serde::de::Visitor<'de> for ConditionSetRefVisitor {
    type Value = ConditionSetRef;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A condition string")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ConditionSetRef::new(s))
    }
}

peg::parser! {
    grammar parser() for str {
        rule _() = [' ' | '\n']*

        rule ron_string() -> String
            = value:$(['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | ')' | '(']+) { value.parse().unwrap() }
            / expected!("ron_string")

        rule ident() -> String
            = value:$(['a'..='z' | 'A'..='Z' | '0'..='9' | '_']+) { value.parse().unwrap() }
            / expected!("ident")

        rule operator() -> Operator
              = "is" { Operator::True }
              / "!is" { Operator::True }
              / "has" { Operator::True }
              / "!has" { Operator::False }
              / expected!("is, !is, has or !has")

        rule set_operator() -> Operator
              = "|" { Operator::Or }
              / "&" { Operator::And }
              / expected!("| or &")

        rule subject() -> Subject
              = "self" { Subject::Me }
              / "target" { Subject::Target }
              / expected!("self or target")

        rule material() -> Material
            = op:operator() _ limit:ron_string() {?
                #[derive(serde::Serialize, serde::Deserialize)]
                struct Wrapper(MaterialLimit);

                ron::de::from_str::<Wrapper>(&format!("Wrapper({})", to_ron_case(&limit)))
                    .map_or(
                        Err("Failed to parse ron value"),
                        |limit|  Ok(Material { op, limit: limit.0})
                    )
            }

        rule nutrition() -> Nutrition
            = "any" { Nutrition::Kind(None) }
            / "calories" { Nutrition::Kind(Some(NeedKind::Calories)) }
            / "hydration" { Nutrition::Kind(Some(NeedKind::Hydration)) }

        rule item() -> Item
            = "name" _ v:ident() { Item::Name(v) }
            / "material" _ v:material() { Item::Material(v) }
            / "property" _ v:ident() {?
                ItemProperty::from_str(&v).map_or(
                    Err("Failed to parse ItemProperty"),
                    |v| Ok(Item::Property(v)))
            }
            / "ability" _ v:ident() {?
                ItemAbilityKind::from_str(&v).map_or(
                    Err("Failed to parse ItemAbilityKind"),
                    |v| Ok(Item::Ability(v)))
            }
            / "nutrition" _ v:nutrition() { Item::Nutrition(v) }
            / v:ident() { Item::Name(v) }
            / expected!("name, property or ability")

        rule property() -> Property
            = v:ident() { Property::Test }

        rule foliage() -> FoliageKind
            = v:ident() {?
                FoliageKind::from_str(&v).map_or(
                    Err("Failed to parse FoliageKind"),
                    Ok)
            }
            / "kind" _ v:ident() {?
                FoliageKind::from_str(&v).map_or(
                    Err("Failed to parse FoliageKind"),
                    Ok)
            }
            / expected!("[kind] or kind [kind]")

        rule tile() -> Tile
            = v:ident() {?
                TileKind::from_str(&v).map_or(
                    Err("Failed to parse TileKind"),
                    |v| Ok(Tile::Kind(v)))
            }
            / "kind" _ v:ident() {?
                TileKind::from_str(&v).map_or(
                    Err("Failed to parse TileKind"),
                    |v| Ok(Tile::Kind(v)))
            }
            / expected!("[kind] or kind [kind]")

        rule value() -> Value
            = "item" _ v:item() { Value::Item(v) }
            / "property" _ v:property() { Value::Property(v) }
            / "tile" _ v:tile() { Value::Tile(v) }
            / "foliage" _ v:foliage() { Value::Foliage(v) }

        rule condition() -> Condition
            = subject:subject() _ op:operator() _ value:value() { Condition { subject, op, value } }
            / op:operator() _ value:value() { Condition { subject: Subject::Me, op, value } }
            / expected!("Failed condition")

        rule condition_set() -> ConditionSet
            = l:condition() _ op:set_operator() _ r:statement() { ConditionSet { left: l, right: Some(ConditionRight { op, set: Box::new(r) } ) } }
            / l:condition() { ConditionSet { left: l, right: None } }
            / expected!("Failed condition_set")

        pub rule statement() -> ConditionSet
            = "(" _ v:condition_set() _ ")" { v }
            /  v:condition_set()


    }
}

pub trait ConditionsParser {
    fn to_condition_set(&self) -> Result<ConditionSet, anyhow::Error>;
}
impl<T> ConditionsParser for T
where
    T: AsRef<str>,
{
    fn to_condition_set(&self) -> Result<ConditionSet, anyhow::Error> {
        parser::statement(self.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to parse condition: {}, ('{}')", e, self.as_ref()))
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condition_parse_test() -> Result<(), anyhow::Error> {
        // "self (has item property is_flammable) | (has item property is_edible) | (has item property is_flammable & (has item property is_edible))";

        println!("{:?}", parser::statement("self has property test").unwrap());

        println!(
            "{:?}",
            parser::statement("self has item property is_flammable").unwrap()
        );
        println!(
            "{:?}",
            parser::statement("self has item ability digging").unwrap()
        );
        println!(
            "{:?}",
            parser::statement("self has item material is Any(Solid)").unwrap()
        );
        // Test upcasing to ron
        println!(
            "{:?}",
            parser::statement("self has item material is any(solid)").unwrap()
        );
        println!(
            "{:?}",
            parser::statement(
                "self has item property is_flammable & self has item property is_flammable"
            )
            .unwrap()
        );
        println!(
            "{:?}",
            parser::statement(
                "self has item property is_flammable & (self has item property is_flammable | self has item property is_flammable)"
            )
            .unwrap()
        );
        println!(
            "{:?}",
            parser::statement(
                "(self has item property is_flammable & (self has item property is_flammable | self has item property is_flammable) )"
            )
                .unwrap()
        );

        Ok(())
    }
}
