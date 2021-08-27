/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! # Delegate object definitions for a `ComponentInterface`.
//!
//! This module converts "interface" definitions from UDL into [`Delegate`] structures
//! that can be added to a `ComponentInterface`.
//!
//! A [`Delegate`] is a collection of methods defined by application code.
//!
//! A declaration in the UDL like this:
//!
//! ```
//! # let ci = uniffi_bindgen::interface::ComponentInterface::from_webidl(r##"
//! # namespace example {};
//! [Delegate]
//! interface ExampleDelegate {
//!   void async_dispatch();
//! };
//! [Delegate=ExampleDelegate]
//! interface Example {
//!   [CallWith=async_dispatch]
//!   void long_running_method();
//! };
//!
//! # "##)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! Will result in an [`Object`] member with one [`Constructor`] and one [`DelegateMethod`] being added
//! to the resulting [`ComponentInterface`]:
//!
//! ```
//! # let ci = uniffi_bindgen::interface::ComponentInterface::from_webidl(r##"
//! # namespace example {};
//! # [Delegate]
//! # interface ExampleDelegate {
//! #   void async_dispatch();
//! # };
//! # "##)?;
//! let obj = ci.get_delegate_definition("ExampleDelegate").unwrap();
//! assert_eq!(obj.name(), "ExampleDelegate");
//! assert_eq!(obj.methods().len(),1 );
//! assert_eq!(obj.methods()[0].name(), "async_dispatch");
//! # Ok::<(), anyhow::Error>(())
//! ```

use std::collections::HashSet;
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};

use anyhow::{bail, Result};

use super::attributes::MethodAttributes;
use super::types::Type;
use super::{APIConverter, ComponentInterface};

/// An "object" is an opaque type that can be instantiated and passed around by reference,
/// have methods called on it, and so on - basically your classic Object Oriented Programming
/// type of deal, except without elaborate inheritence hierarchies.
///
/// In UDL these correspond to the `interface` keyword.
///
/// At the FFI layer, objects are represented by an opaque integer handle and a set of functions
/// a common prefix. The object's constuctors are functions that return new objects by handle,
/// and its methods are functions that take a handle as first argument. The foreign language
/// binding code is expected to stitch these functions back together into an appropriate class
/// definition (or that language's equivalent thereof).
///
/// TODO:
///  - maybe "Class" would be a better name than "Object" here?
#[derive(Debug, Clone)]
pub struct Delegate {
    pub(super) name: String,
    pub(super) methods: Vec<DelegateMethod>,
}

impl Delegate {
    fn new(name: String) -> Self {
        Self {
            name,
            methods: Default::default(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn type_(&self) -> Type {
        Type::DelegateObject(self.name.clone())
    }

    pub fn methods(&self) -> Vec<&DelegateMethod> {
        self.methods.iter().collect()
    }
}

impl Hash for Delegate {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.methods.hash(state);
    }
}

impl APIConverter<Delegate> for weedle::InterfaceDefinition<'_> {
    fn convert(&self, ci: &mut ComponentInterface) -> Result<Delegate> {
        if self.inheritance.is_some() {
            bail!("interface inheritence is not supported");
        }
        let mut delegate = Delegate::new(self.identifier.0.to_string());
        // Convert each member into a constructor or method, guarding against duplicate names.
        let mut member_names = HashSet::new();
        for member in &self.members.body {
            match member {
                weedle::interface::InterfaceMember::Operation(t) => {
                    let mut method: DelegateMethod = t.convert(ci)?;
                    if !member_names.insert(method.name.clone()) {
                        bail!("Duplicate interface member name: \"{}\"", method.name())
                    }
                    method.object_name = delegate.name.clone();
                    delegate.methods.push(method);
                }
                _ => bail!("no support for interface member type {:?} yet", member),
            }
        }
        Ok(delegate)
    }
}

// Represents an instance method for an object type.
//
// The FFI will represent this as a function whose first/self argument is a
// `FFIType::RustArcPtr` to the instance.
#[derive(Debug, Clone)]
pub struct DelegateMethod {
    pub(super) name: String,
    pub(super) object_name: String,
    pub(super) return_type: Option<Type>,
    pub(super) attributes: MethodAttributes,
}

impl DelegateMethod {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn return_type(&self) -> Option<Type> {
        self.return_type.to_owned()
    }

    pub fn returns(&self) -> bool {
        self.return_type.is_some()
    }

    pub fn throws(&self) -> Option<&str> {
        self.attributes.get_throws_err()
    }

    pub fn throws_type(&self) -> Option<Type> {
        self.attributes
            .get_throws_err()
            .map(|name| Type::Error(name.to_owned()))
    }
}

// impl IterTypes for DelegateMethod {
//     fn iter_types(&self) -> TypeIterator<'_> {
//         // XXX Not sure that this is needed at all here.
//         Box::new(
//             Default::default(),
//         )
//     }
// }

impl Hash for DelegateMethod {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.return_type.hash(state);
        self.attributes.hash(state);
    }
}

impl APIConverter<DelegateMethod> for weedle::interface::OperationInterfaceMember<'_> {
    fn convert(&self, ci: &mut ComponentInterface) -> Result<DelegateMethod> {
        if self.special.is_some() {
            bail!("special operations not supported");
        }
        if self.modifier.is_some() {
            bail!("method modifiers are not supported")
        }
        if !self.args.body.list.is_empty() {
            bail!("custom method arguments are not supported")
        }
        // Delegate methods should only return `any` or `void`,
        // e.g. transparently returning the delegated method's return type, or swallowing it.
        // XXX At the moment, `any` is not recognized.
        let return_type = ci.resolve_return_type_expression(&self.return_type)?;
        Ok(DelegateMethod {
            name: match self.identifier {
                None => bail!("anonymous methods are not supported {:?}", self),
                Some(id) => {
                    let name = id.0.to_string();
                    if name == "new" {
                        bail!("the method name \"new\" is reserved for the default constructor");
                    }
                    name
                }
            },
            // We don't know the name of the containing `Object` at this point, fill it in later.
            object_name: Default::default(),
            return_type,
            attributes: MethodAttributes::try_from(self.attributes.as_ref())?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_that_all_argument_and_return_types_become_known() {
        const UDL: &str = r#"
            namespace test{};
            [Delegate]
            interface Testing {
                sequence<u32> code_points_of_name();
            };
        "#;
        let ci = ComponentInterface::from_webidl(UDL).unwrap();
        assert_eq!(ci.iter_delegate_definitions().len(), 1);
        ci.get_delegate_definition("Testing").unwrap();

        // assert_eq!(ci.iter_types().len(), 6);
        // assert!(ci.iter_types().iter().any(|t| t.canonical_name() == "u16"));
        // assert!(ci.iter_types().iter().any(|t| t.canonical_name() == "u32"));
        // assert!(ci
        //     .iter_types()
        //     .iter()
        //     .any(|t| t.canonical_name() == "Sequenceu32"));
        // assert!(ci
        //     .iter_types()
        //     .iter()
        //     .any(|t| t.canonical_name() == "string"));
        // assert!(ci
        //     .iter_types()
        //     .iter()
        //     .any(|t| t.canonical_name() == "Optionalstring"));
        // assert!(ci
        //     .iter_types()
        //     .iter()
        //     .any(|t| t.canonical_name() == "TypeTesting"));
    }

    #[test]
    fn test_the_name_new_is_reserved_for_constructors() {
        const UDL: &str = r#"
            namespace test{};
            interface Testing {
                void new(u32 v);
            };
        "#;
        let err = ComponentInterface::from_webidl(UDL).unwrap_err();
        assert_eq!(
            err.to_string(),
            "the method name \"new\" is reserved for the default constructor"
        );
    }
}
