/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::obj::{GodotClass, InstanceId};

// This is private; despite `pub` here it is re-exported in `crate::private` module.

/// Object runtime type information, obtained at creation time.
///
/// Stores how a Godot-managed object has been created, for debug info and runtime checks.
/// This is persisted independently of the static type system (e.g. `T` in `Gd<T>`) and can be used to perform sanity checks at runtime.
///
/// See also <https://github.com/godot-rust/gdext/issues/23>.
#[derive(Debug)]
pub struct ObjectRtti {
    /// Cached instance ID. May point to dead objects.
    pub instance_id: InstanceId,

    /// Only in Debug mode: dynamic class.
    #[cfg(debug_assertions)]
    pub class_name: crate::builtin::meta::ClassName,
}

impl ObjectRtti {
    /// Creates a new instance of `ObjectRtti`.
    #[inline]
    pub fn of<T: GodotClass>(instance_id: InstanceId) -> Self {
        Self {
            instance_id,

            #[cfg(debug_assertions)]
            class_name: T::class_name(),
        }
    }

    /// Checks that the object is of type `T` or derived. Returns instance ID.
    ///
    /// # Panics
    /// In Debug mode, if the object is not of type `T` or derived.
    #[inline]
    pub fn check_type<T: GodotClass>(&self) -> InstanceId {
        #[cfg(debug_assertions)]
        crate::engine::ensure_object_inherits(self.class_name, T::class_name(), self.instance_id);

        self.instance_id
    }
}
