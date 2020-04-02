#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    non_upper_case_globals
)]
extern crate proc_macro;

use crate::proc_macro::TokenStream;
use proc_macro_hack::proc_macro_hack;
use quote::quote;

#[proc_macro_hack]
pub fn fnv(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::LitStr);

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.value().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }

    let gen = quote! {
        { #hash }
    };

    gen.into()
}

struct DefinitionSettings {
    loader: proc_macro2::TokenStream,
    resolver: proc_macro2::TokenStream,
    component: Option<proc_macro2::TokenStream>,
}
impl Default for DefinitionSettings {
    fn default() -> Self {
        Self {
            loader: quote! { crate::defs::DefaultDefinitionLoader<Self> },
            resolver: quote! { crate::defs::DefaultDefinitionResolver<Self> },
            component: None,
        }
    }
}

#[allow(clippy::too_many_lines)]
#[proc_macro_derive(Definition, attributes(definition))]
pub fn definition_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemStruct);
    let name = &input.ident;

    let mut settings = DefinitionSettings::default();

    input.attrs.iter().for_each(|attr| {
        if attr.path.is_ident("definition") {
            let meta = attr.parse_meta().unwrap();

            if let syn::Meta::List(list) = meta {
                list.nested.iter().for_each(|nested| {
                    if let syn::NestedMeta::Meta(meta) = nested {
                        if let syn::Meta::NameValue(kv) = meta {
                            if kv.path.is_ident("loader") {
                                if let syn::Lit::Str(s) = &kv.lit {
                                    let custom_loader = proc_macro2::Ident::new(
                                        s.value().as_str(),
                                        proc_macro2::Span::call_site(),
                                    );
                                    settings.loader = quote! { #custom_loader };
                                }
                            } else if kv.path.is_ident("resolver") {
                                if let syn::Lit::Str(s) = &kv.lit {
                                    let custom_loader = proc_macro2::Ident::new(
                                        s.value().as_str(),
                                        proc_macro2::Span::call_site(),
                                    );
                                    settings.resolver = quote! { #custom_loader };
                                }
                            } else if kv.path.is_ident("component") {
                                if let syn::Lit::Str(s) = &kv.lit {
                                    let custom_component = proc_macro2::Ident::new(
                                        s.value().as_str(),
                                        proc_macro2::Span::call_site(),
                                    );
                                    settings.component = Some(quote! { #custom_component });
                                }
                            }
                        }
                    }
                });
            }
        };
    });

    let id_type_name =
        proc_macro2::Ident::new(&format!("{}Id", name), proc_macro2::Span::call_site());

    let component_type_name = proc_macro2::Ident::new(
        &name.to_string().replace("Definition", "Component"),
        proc_macro2::Span::call_site(),
    );

    let ref_type_name = proc_macro2::Ident::new(
        &name.to_string().replace("Definition", "Ref"),
        proc_macro2::Span::call_site(),
    );

    let visitor_type_name = proc_macro2::Ident::new(
        &name.to_string().replace("Definition", "Visitor"),
        proc_macro2::Span::call_site(),
    );

    let mut gen = quote! {
        #[derive(shrinkwraprs::Shrinkwrap, Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[shrinkwrap(mutable)]
        pub struct #id_type_name(pub usize);
        impl #id_type_name {
            pub fn fetch<'a>(&self, storage: &'a crate::defs::DefinitionStorage<#name>) -> &'a #name {
                storage.get(*self).unwrap()
            }
        }
        #[derive(Debug)]
        pub struct #ref_type_name {
            name: String,
            id: Option<#id_type_name>,
            value: Option<std::sync::Arc<#name>>,
        }
        impl Clone for #ref_type_name {
            fn clone(&self) -> Self {
                Self {
                    name: self.name.clone(),
                    id: self.id,
                    value: None,
                }
            }
        }
        impl #ref_type_name {
            pub fn new(name: &str) -> Self {
                Self {
                    name: name.to_string(),
                    id: None,
                    value: None,
                }
            }

            pub fn resolved(&self) -> bool {
                self.id.is_some()
            }

            pub(crate) fn clone_ptr(&self) -> std::sync::Arc<#name> {
                self.value.as_ref().unwrap().clone()
            }

            pub fn value(&self) -> &#name {
                &self.value.as_ref().unwrap()
            }

            pub fn id(&self) -> #id_type_name {
                self.id.unwrap()
            }

            pub(crate) fn clone(&self) -> Self {
                Self {
                    name: self.name.clone(),
                    id: self.id,
                    value: self.value.clone(),
                }
            }

            pub fn resolve(&mut self, storage: &crate::defs::DefinitionStorage<#name>) -> Result<(),anyhow::Error> {
                self.id = Some(storage.get_id(&self.name).ok_or(anyhow::anyhow!("Invalid definition ref: type={}, {}", std::any::type_name::<#name>(), self.name))?);

                self.value = storage.get_raw(self.id.unwrap()).cloned();

                Ok(())
            }

             pub fn fetch<'a>(&self, storage: &'a crate::defs::DefinitionStorage<#name>) -> Option<&'a #name> {
                self.id.map(|id| id.fetch(storage))
            }
        }

        impl std::ops::Deref for #ref_type_name {
            type Target = #name;

            fn deref(&self) -> &Self::Target {
                self.value.as_ref().unwrap()
            }
        }

        impl PartialEq for #ref_type_name {
            fn eq(&self, other: &Self) -> bool {
                if let Some(self_id) = self.id {
                    if let Some(other_id) = other.id {
                        return self_id == other_id
                    }
                }

                self.name == other.name
            }
        }
        impl PartialEq<#component_type_name> for #ref_type_name {
            fn eq(&self, other: &#component_type_name) -> bool {
                if let Some(self_id) = self.id {
                    return self_id == other.def;
                }

                panic!("Trying to compare unresolved ref")
            }
        }
        impl Eq for #ref_type_name {}
        impl std::hash::Hash for #ref_type_name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                 self.name.hash(state);
            }
        }

        impl<S> From<S> for #ref_type_name
            where S: AsRef<str> {
            fn from(name: S) -> Self {
                Self::new(name.as_ref())
            }
        }

        impl serde::Serialize for #ref_type_name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.name)
            }
        }
        impl<'de> serde::Deserialize<'de> for #ref_type_name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<#ref_type_name, D::Error> {
                deserializer.deserialize_str(#visitor_type_name)
            }
        }
        struct #visitor_type_name;
        impl<'de> serde::de::Visitor<'de> for #visitor_type_name {
            type Value = #ref_type_name;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result { formatter.write_str("A string ref name") }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(#ref_type_name::new(s))
            }
        }

        impl crate::defs::DefinitionId for #id_type_name {
            type Definition = #name;

            fn as_str<'a>(&self, storage: &'a crate::defs::DefinitionStorage<Self::Definition>) -> &'a str {
                storage.get(*self).unwrap().details.name.as_str()
            }
        }
        impl From<u16> for #id_type_name { fn from(other: u16) -> Self { Self(other as usize) } }
        impl Into<u16> for #id_type_name { fn into(self) -> u16 { self.0 as u16 } }
        impl From<usize> for #id_type_name { fn from(other: usize) -> Self { Self(other) } }
        impl From<#id_type_name> for usize { fn from(other: #id_type_name) -> Self { *other } }

        impl PartialEq for #name {
            fn eq(&self, other: &Self) -> bool {
                self.id.eq(&other.id)
            }
        }
    };

    let DefinitionSettings {
        loader,
        resolver,
        component,
    } = settings;

    let component = if let Some(component) = component {
        quote! { #component  }
    } else {
        /*let uuid = uuid::Uuid::new_v5(
             &uuid::Uuid::NAMESPACE_DNS,
             component_type_name.to_string().as_bytes(),
         );
        let uuid_str = syn::LitStr::new(
             &uuid.to_hyphenated().to_string(),
             proc_macro2::Span::call_site(),
         );*/

        gen = quote! {
            #gen

            #[derive(type_uuid::TypeUuid, Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
            pub struct #component_type_name {
                def: #id_type_name,
            }
            impl #component_type_name {
                pub fn new(def: <#name as crate::defs::Definition>::Id) -> Self {
                    Self { def }
                }
            }
            impl crate::defs::DefinitionComponent<#name> for #component_type_name {
                fn id(&self) -> <#name as crate::defs::Definition>::Id {
                    self.def
                }

                fn fetch<'a>(&self, storage: &'a crate::defs::DefinitionStorage<#name>) -> &'a #name {
                    self.def.fetch(storage)
                }
            }
        };
        quote! { #component_type_name }
    };

    gen = quote! {
        #gen

         impl crate::defs::Definition for #name {
            type Id = #id_type_name;
            type Component = #component;
            type Loader = #loader;
            type Resolver = #resolver;

            fn name(&self) -> &str {
                &self.details.name
            }
            fn display_name(&self) -> &str {
                if self.details.display_name.is_empty() {
                    &self.details.name
                } else {
                    &self.details.display_name
                }
            }
            fn description(&self) -> &str {
                 &self.details.description
            }
            fn long_description(&self) -> &str {
                if self.details.long_description.is_empty() {
                    &self.details.description
                } else {
                    &self.details.long_description
                }
            }

            fn details(&self) -> &DefinitionDetails {
                &self.details
            }
            fn id(&self) -> #id_type_name {
                self.id
            }
            fn set_id(&mut self, id: #id_type_name) {
                self.id = id;
            }
        }
    };

    gen.into()
}
