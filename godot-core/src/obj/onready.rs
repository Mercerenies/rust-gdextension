/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::property::{Export, Property, PropertyHintInfo};
use std::mem;

/// Ergonomic late-initialization container with `ready()` support.
///
/// While deferred initialization is generally seen as bad practice, it is often inevitable in game development.
/// Godot in particular encourages initialization inside `ready()`, e.g. to access the scene tree after a node is inserted into it.
/// The alternative to using this pattern is [`Option<T>`][option], which needs to be explicitly unwrapped with `unwrap()` or `expect()` each time.
///
/// `OnReady<T>` should always be used as a field. There are two modes to use it:
///
/// 1. **Automatic mode, using [`new()`](Self::new).**<br>
///    Before `ready()` is called, all `OnReady` fields constructed with `new()` are automatically initialized, in the order of
///    declaration. This means that you can safely access them in `ready()`.<br><br>
/// 2. **Manual mode, using [`manual()`](Self::manual).**<br>
///    These fields are left uninitialized until you call [`init()`][Self::init] on them. This is useful if you need more complex
///    initialization scenarios than a closure allows. If you forget initialization, a panic will occur on first access.
///
/// Conceptually, `OnReady<T>` is very close to [once_cell's `Lazy<T>`][lazy], with additional hooks into the Godot lifecycle.
/// The absence of methods to check initialization state is deliberate: you don't need them if you follow the above two patterns.
/// This container is not designed as a general late-initialization solution, but tailored to the `ready()` semantics of Godot.
///
/// This type is not thread-safe. `ready()` runs on the main thread and you are expected to access its value on the main thread, as well.
///
/// [option]: std::option::Option
/// [lazy]: https://docs.rs/once_cell/1/once_cell/unsync/struct.Lazy.html
///
/// # Example
/// ```
/// use godot::prelude::*;
///
/// #[derive(GodotClass)]
/// #[class(base = Node)]
/// struct MyClass {
///    auto: OnReady<i32>,
///    manual: OnReady<i32>,
/// }
///
/// #[godot_api]
/// impl INode for MyClass {
///     fn init(_base: Base<Node>) -> Self {
///        Self {
///            auto: OnReady::new(|| 11),
///            manual: OnReady::manual(),
///        }
///     }
///
///     fn ready(&mut self) {
///        // self.auto is now ready with value 11.
///        assert_eq!(*self.auto, 11);
///
///        // self.manual needs to be initialized manually.
///        self.manual.init(22);
///        assert_eq!(*self.manual, 22);
///     }
/// }
pub struct OnReady<T> {
    state: InitState<T>,
}

impl<T> OnReady<T> {
    /// Schedule automatic initialization before `ready()`.
    ///
    /// This guarantees that the value is initialized once `ready()` starts running.
    /// Until then, accessing the object may panic. In particular, the object is _not_ initialized on first use.
    ///
    /// The value is also initialized when you don't override `ready()`.
    ///
    /// For more control over initialization, use the [`OnReady::manual()`] constructor, followed by a [`self.init()`][OnReady::init]
    /// call during `ready()`.
    pub fn new<F>(init_fn: F) -> Self
    where
        F: FnOnce() -> T + 'static,
    {
        Self {
            state: InitState::AutoPrepared {
                initializer: Box::new(init_fn),
            },
        }
    }

    /// Leave uninitialized, expects manual initialization during `ready()`.
    ///
    /// If you use this method, you _must_ call [`init()`][Self::init] during the `ready()` callback, otherwise a panic will occur.
    pub fn manual() -> Self {
        Self {
            state: InitState::ManualUninitialized,
        }
    }

    /// Runs manual initialization.
    ///
    /// # Panics
    /// - If `init()` was called before.
    /// - If this object was already provided with a closure during construction, in [`Self::new()`].
    pub fn init(&mut self, value: T) {
        match &self.state {
            InitState::ManualUninitialized { .. } => {
                self.state = InitState::Initialized { value };
            }
            InitState::AutoPrepared { .. } => {
                panic!("cannot call init() on auto-initialized OnReady objects")
            }
            InitState::AutoInitializing => {
                // SAFETY: Loading is ephemeral state that is only set in init_auto() and immediately overwritten.
                unsafe { std::hint::unreachable_unchecked() }
            }
            InitState::Initialized { .. } => {
                panic!("already initialized; did you call init() more than once?")
            }
        };
    }

    /// Runs initialization.
    ///
    /// # Panics
    /// If the value is already initialized.
    pub(crate) fn init_auto(&mut self) {
        // Two branches needed, because mem::replace() could accidentally overwrite an already initialized value.
        match &self.state {
            InitState::ManualUninitialized => return, // skipped
            InitState::AutoPrepared { .. } => {}      // handled below
            InitState::AutoInitializing => {
                // SAFETY: Loading is ephemeral state that is only set below and immediately overwritten.
                unsafe { std::hint::unreachable_unchecked() }
            }
            InitState::Initialized { .. } => panic!("OnReady object already initialized"),
        };

        // Temporarily replace with dummy state, as it's not possible to take ownership of the initializer closure otherwise.
        let InitState::AutoPrepared { initializer } =
            mem::replace(&mut self.state, InitState::AutoInitializing)
        else {
            // SAFETY: condition checked above.
            unsafe { std::hint::unreachable_unchecked() }
        };

        self.state = InitState::Initialized {
            value: initializer(),
        };
    }
}

// Panicking Deref is not best practice according to Rust, but constant get() calls are significantly less ergonomic and make it harder to
// migrate between T and LateInit<T>, because all the accesses need to change.
impl<T> std::ops::Deref for OnReady<T> {
    type Target = T;

    /// Returns a shared reference to the value.
    ///
    /// # Panics
    /// If the value is not yet initialized.
    fn deref(&self) -> &Self::Target {
        match &self.state {
            InitState::ManualUninitialized => {
                panic!("OnReady manual value uninitialized, did you call init()?")
            }
            InitState::AutoPrepared { .. } => {
                panic!("OnReady automatic value uninitialized, is only available in ready()")
            }
            InitState::AutoInitializing => unreachable!(),
            InitState::Initialized { value } => value,
        }
    }
}

impl<T> std::ops::DerefMut for OnReady<T> {
    /// Returns an exclusive reference to the value.
    ///     
    /// # Panics
    /// If the value is not yet initialized.
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.state {
            InitState::Initialized { value } => value,
            InitState::ManualUninitialized { .. } | InitState::AutoPrepared { .. } => {
                panic!("value not yet initialized")
            }
            InitState::AutoInitializing => unreachable!(),
        }
    }
}

impl<T: Property> Property for OnReady<T> {
    type Intermediate = T::Intermediate;

    fn get_property(&self) -> Self::Intermediate {
        let deref: &T = self;
        deref.get_property()
    }

    fn set_property(&mut self, value: Self::Intermediate) {
        let deref: &mut T = self;
        deref.set_property(value);
    }

    fn property_hint() -> PropertyHintInfo {
        T::property_hint()
    }
}

impl<T: Export> Export for OnReady<T> {
    fn default_export_info() -> PropertyHintInfo {
        T::default_export_info()
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Implementation

enum InitState<T> {
    ManualUninitialized,
    AutoPrepared { initializer: Box<dyn FnOnce() -> T> },
    AutoInitializing, // needed because state cannot be empty
    Initialized { value: T },
}
