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
use super::types::{ReturnType, Type};
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
pub struct DelegateObject {
    pub(super) name: String,
    pub(super) methods: Vec<DelegateMethod>,
}

impl DelegateObject {
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

    pub fn find_method(&self, nm: &str) -> Option<&DelegateMethod> {
        self.methods.iter().find(|m| m.name == nm)
    }
}

impl Hash for DelegateObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.methods.hash(state);
    }
}

impl APIConverter<DelegateObject> for weedle::InterfaceDefinition<'_> {
    fn convert(&self, ci: &mut ComponentInterface) -> Result<DelegateObject> {
        if self.inheritance.is_some() {
            bail!("interface inheritence is not supported");
        }
        let mut delegate = DelegateObject::new(self.identifier.0.to_string());
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
    pub(super) return_type: ReturnType,
    pub(super) attributes: MethodAttributes,
}

impl DelegateMethod {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn return_type(&self) -> &ReturnType {
        &self.return_type
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

    use super::super::object::{Method, Object};

    #[test]
    fn test_delegate_attribute_makes_a_delegate_object() {
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
    }

    #[test]
    fn test_the_name_new_is_reserved_for_constructors() {
        const UDL: &str = r#"
            namespace test{};
            [Delegate]
            interface Testing {
                void new();
            };
        "#;
        let err = ComponentInterface::from_webidl(UDL).unwrap_err();
        assert_eq!(
            err.to_string(),
            "the method name \"new\" is reserved for the default constructor"
        );
    }

    #[test]
    fn test_methods_have_zero_args() {
        const UDL: &str = r#"
            namespace test{};
            [Delegate]
            interface Testing {
                void method(u32 arg);
            };
        "#;
        let err = ComponentInterface::from_webidl(UDL).unwrap_err();
        assert_eq!(err.to_string(), "custom method arguments are not supported");
    }

    #[test]
    fn test_delegate_methods_can_throw() {
        const UDL: &str = r#"
            namespace test{};
            [Delegate]
            interface Testing {
                [Throws=Error]
                void method();
            };

            [Error]
            enum Error {
                "LOLWUT"
            };
        "#;
        let ci = ComponentInterface::from_webidl(UDL).unwrap();
        let dobj = ci.get_delegate_definition("Testing").unwrap();
        let m = dobj.find_method("method").unwrap();

        assert_eq!(m.throws_type(), Some(Type::Error("Error".into())));
    }

    #[test]
    fn test_delegate_methods_can_override_return_types_and_throw_types() {
        const UDL: &str = r#"
            namespace test{};
            [Delegate]
            interface TheDelegate {
                [Throws=Error]
                void it_throws();

                void it_swallows();

                i32 it_counts();

                any it_passes_through();
            };

            [Delegate=TheDelegate]
            interface Testing {
                [Throws=Error, CallWith=it_swallows]
                void thrower();

                [CallWith=it_throws]
                void silent();

                [CallWith=it_counts]
                void counted();

                [CallWith=it_passes_through]
                sequence<i32?> exotic();
            };

            [Error]
            enum Error {
                "LOLWUT"
            };
        "#;
        let ci = ComponentInterface::from_webidl(UDL).unwrap();
        let dobj = ci.get_delegate_definition("TheDelegate");
        let obj = ci.get_object_definition("Testing").unwrap();

        fn find_method<'a>(nm: &str, obj: &'a Object) -> &'a Method {
            obj.methods.iter().find(|m| m.name() == nm).unwrap()
        }

        let m = find_method("thrower", obj);
        // thrower delegates through it_swallows, which returns void and throws nothing
        assert_eq!(m.delegated_return_type(&dobj), None);
        assert_eq!(m.delegated_throws_type(&dobj), None);

        let m = find_method("silent", obj);
        // silent delegates through it_throws, which returns void and throws nothing
        assert_eq!(m.delegated_return_type(&dobj), None);
        assert_eq!(
            m.delegated_throws_type(&dobj),
            Some(Type::Error("Error".into()))
        );

        let m = find_method("counted", obj);
        // counted delegates through it_counts, which returns i32 and throws nothing
        assert_eq!(m.delegated_return_type(&dobj), Some(Type::Int32));
        assert_eq!(m.delegated_throws_type(&dobj), None);

        let m = find_method("exotic", obj);
        // exotic delegates through it_passes_through, which returns Sequence<Option<i32>> and throws nothing
        assert_eq!(
            m.delegated_return_type(&dobj),
            Some(Type::Sequence(Box::new(Type::Optional(Box::new(
                Type::Int32
            )))))
        );
        assert_eq!(m.delegated_throws_type(&dobj), None);
    }
}
