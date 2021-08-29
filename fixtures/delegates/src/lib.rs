/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[derive(Debug, Clone)]
pub struct RustObject;

impl RustObject {
    fn new() -> Self {
        Self
    }

    fn from_string(_s: String) -> Self {
        Self
    }

    fn identity_string(&self, s: String) -> String {
        s
    }
}

include!(concat!(env!("OUT_DIR"), "/delegates.uniffi.rs"));
