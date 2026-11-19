//! Plugins that describe themselves, so that a program configuring them never names one.
//!
//! A plugin, see [`PluginType`], says what to call it, what it has to be told in order to be
//! built, see [`schema`] and [`config`], and what it provides to and requires of the composite it
//! is part of, see [`compatibility`]. It registers itself in the [`catalog`], which is where a
//! program looking for a plugin finds it. A [`Draft`](draft::Draft) holds a composite of
//! plugins while it is being configured.
//!
//! An axis of plugins is a trait declared with [`axis`]. A plugin is usually derived, which reads
//! its schema from its fields and their `serde` attributes, and registered with [`register`],
//! which tags it for `typetag` with its id. A composite derives its schema the same way, see
//! [`derive@schema::Configurable`]:
//!
//! ```
//! use plugin_factory::{Plugin, axis, register, schema::Configurable};
//! use serde::{Deserialize, Serialize};
//!
//! #[axis]
//! pub trait Seeding: Plugin {}
//!
//! #[derive(Debug, Serialize, Deserialize, Plugin)]
//! #[plugin(id = "my_crate::random", label = "random")]
//! struct Random {
//!     #[plugin(label = "Keep the top seeds", default = 0)]
//!     keep_top: u32,
//! }
//!
//! #[register]
//! impl Seeding for Random {}
//!
//! #[derive(Deserialize, Configurable)]
//! struct Phase {
//!     #[plugin(label = "Seeding")]
//!     seeding: Box<dyn Seeding>,
//! }
//! # fn main() {
//! # use plugin_factory::catalog::AnyAxis;
//! # assert_eq!(
//! #     AnyAxis::of::<dyn Seeding>().entry("my_crate::random").unwrap().label(),
//! #     "random"
//! # );
//! # assert_eq!(Phase::schema().fields()[0].name(), "seeding");
//! # }
//! ```

use std::fmt::Debug;

use crate::{compatibility::Declaration, schema::Configurable};

// so that the code the derive generates for the crate's own tests names it as any other crate does
extern crate self as plugin_factory;

pub mod catalog;
pub mod compatibility;
pub mod config;
pub mod draft;
mod phrase;
pub mod schema;

pub use plugin_factory_derive::{Plugin, axis, register};

pub use crate::phrase::Phrase;

/// What the code [`derive@Plugin`], [`axis`] and [`register`] generate refers to, and what a
/// plugin behind a `dyn` is asked through. Not part of the API.
#[doc(hidden)]
pub mod __private {
    use std::{collections::BTreeMap, sync::OnceLock};

    // so that a crate declaring an axis or registering a plugin does not have to depend on them
    pub use inventory;
    pub use serde;
    pub use serde_json::value::Serializer;

    use crate::{
        Plugin, PluginType,
        catalog::{Axis, PluginEntry},
        compatibility::{Declaration, OpenFacts},
        schema::Configurable,
    };

    /// What is known of one axis: the plugins registered for it, and the facts they could
    /// provide, each built on first access. Held in a static by the code [`axis`](crate::axis)
    /// generates.
    pub struct Index {
        registered: fn() -> Vec<&'static PluginEntry>,
        entries: OnceLock<Entries>,
        pub(crate) facts: OnceLock<OpenFacts>,
    }

    /// The plugins registered for an axis.
    pub(crate) struct Entries {
        /// Every version of every plugin, by [`PluginType::ID`].
        pub(crate) all: BTreeMap<&'static str, &'static PluginEntry>,
        /// The latest version of every plugin, by the id its versions share, see
        /// [`unversioned`](crate::unversioned).
        pub(crate) latest: BTreeMap<&'static str, &'static PluginEntry>,
    }

    impl Index {
        /// The index of the plugins `registered` returns.
        pub const fn new(registered: fn() -> Vec<&'static PluginEntry>) -> Self {
            Self {
                registered,
                entries: OnceLock::new(),
                facts: OnceLock::new(),
            }
        }

        /// The registered plugins.
        ///
        /// # Panics
        /// Panics if two of them share an id, or an id does not say the version it is of, see
        /// [`PluginType::ID`].
        pub(crate) fn entries(&self) -> &Entries {
            self.entries.get_or_init(|| {
                let mut all = BTreeMap::new();
                let mut latest = BTreeMap::<_, &PluginEntry>::new();
                for entry in (self.registered)() {
                    let expected = match entry.version() {
                        1 => entry.unversioned_id().to_owned(),
                        version => format!(
                            "{}{}{version}",
                            entry.unversioned_id(),
                            crate::VERSION_SEPARATOR
                        ),
                    };
                    assert!(
                        entry.version() > 0 && entry.id() == expected,
                        "the plugin {} of version {} has the id {:?} rather than {expected:?}",
                        entry.type_name(),
                        entry.version(),
                        entry.id(),
                    );
                    if let Some(other) = all.insert(entry.id(), entry) {
                        panic!(
                            "the plugins {} and {} are both registered with the id {:?}",
                            other.type_name(),
                            entry.type_name(),
                            entry.id(),
                        );
                    }
                    latest
                        .entry(entry.unversioned_id())
                        .and_modify(|other| {
                            if entry.version() > other.version() {
                                *other = entry;
                            }
                        })
                        .or_insert(entry);
                }
                Entries { all, latest }
            })
        }
    }

    /// The entry of the plugin `P` for the axis `A`, which [`register`](crate::register)
    /// submits.
    pub const fn entry<P: PluginType, A: Axis + ?Sized>() -> PluginEntry {
        PluginEntry::new::<P, A>()
    }

    /// How the plugin `P` is registered for an axis: the value of the axis's registry that
    /// [`register`](crate::register) submits. Implemented by [`axis`](crate::axis) for every
    /// plugin of the axis.
    pub trait Register<P>: Axis {
        /// The registry of the axis, a type of the crate declaring it.
        type Registry: 'static;

        /// The entry of `P`.
        const REGISTRY: Self::Registry;
    }

    /// What only the type of a plugin knows, asked of a plugin behind a `dyn`. Implemented for
    /// every [`PluginType`].
    pub trait Erased {
        /// The plugin's [`PluginType::ID`].
        fn id(&self) -> &'static str;

        /// The plugin as a plugin of no particular axis.
        fn as_plugin(&self) -> &dyn Plugin;

        /// The plugin as a value of its type, see [`downcast_ref`](crate::downcast_ref).
        fn as_any(&self) -> &dyn core::any::Any;

        /// Adds everything the plugin declares to `declaration`: its static declaration, what its
        /// configuration declares, and then what its sub-plugins declare, recursively.
        fn declare_all(&self, declaration: &mut Declaration);

        /// The plugins held by its fields, see [`Configurable::plugins`].
        fn sub_plugins(&self) -> Vec<&dyn Plugin>;
    }

    impl<T: PluginType> Erased for T {
        fn id(&self) -> &'static str {
            T::ID
        }

        fn as_plugin(&self) -> &dyn Plugin {
            self
        }

        fn as_any(&self) -> &dyn core::any::Any {
            self
        }

        // qualified throughout, since a plugin may have inherent functions of the same names
        fn declare_all(&self, declaration: &mut Declaration) {
            <T as PluginType>::declare_static(declaration);
            Plugin::declare(self, declaration);
            for sub_plugin in Configurable::plugins(self) {
                sub_plugin.declare_all(declaration);
            }
        }

        fn sub_plugins(&self) -> Vec<&dyn Plugin> {
            Configurable::plugins(self)
        }
    }
}

/// A plugin of some axis as it is asked behind a `dyn`, see [`catalog`]: what its configuration
/// declares. Everything about its type is in [`PluginType`].
///
/// Usually derived, see [`derive@Plugin`]. The [`Debug`] form is what names the plugin in an
/// [`Incompatibility`](compatibility::Incompatibility).
pub trait Plugin: Debug + __private::Erased + 'static {
    /// Declares the facts this configuration provides and requires on top of
    /// [`PluginType::declare_static`], and the values of the facts declared there; nothing by
    /// default.
    ///
    /// The declaration must depend on the plugin's configuration alone, since it is made before
    /// the composite exists.
    fn declare(&self, _: &mut Declaration) {}
}

/// What a plugin is as a type: what identifies and describes it, and what every configuration of
/// it declares. Usually derived, see [`derive@Plugin`].
///
/// A plugin is configured through its [`Configurable`] impl, which is also where the plugins it
/// is made of come from.
pub trait PluginType: Plugin + Configurable {
    /// What identifies this version of the plugin among the plugins of its axis, in a config as
    /// in a saved file, e.g. `"my_crate::points"`. Never change it once configurations naming it
    /// are saved.
    ///
    /// The first version's id is the plugin's id alone, a later one's the id followed by
    /// [`VERSION_SEPARATOR`] and its [version](Self::VERSION), e.g. `"my_crate::points@2"`, which
    /// [`derive@Plugin`] writes; see [`unversioned`]. The catalog panics on any other id.
    ///
    /// A plugin persisted through `typetag` has to be tagged with this id, which [`register`]
    /// does.
    const ID: &'static str;

    /// Which version of the plugin this is, counted from 1.
    ///
    /// A version is a type of its own, so that what was saved with one keeps behaving as it did
    /// once a later one behaves differently. All versions of a plugin share its texts; the catalog
    /// offers only the latest, and finds the others by their ids, see
    /// [`AnyAxis::entries`](catalog::AnyAxis::entries) and
    /// [`AnyAxis::successor`](catalog::AnyAxis::successor).
    const VERSION: u32 = 1;

    /// What an operator picking a plugin reads.
    const LABEL: &'static str;

    /// What an operator picking a plugin reads about what it does; nothing by default.
    const DESCRIPTION: Option<&'static str> = None;

    /// Declares the facts every configuration of this plugin provides to and requires of the
    /// composite it is part of, see [`Declaration`]; nothing by default.
    ///
    /// A program reads this declaration from the [catalog] before the plugin is configured.
    fn declare_static(_: &mut Declaration) {}
}

/// What separates a plugin's id from its version in the [`PluginType::ID`] of a later version.
pub const VERSION_SEPARATOR: char = '@';

/// The id all versions of the plugin `id`, a [`PluginType::ID`], share: `id` without its version.
///
/// ```
/// assert_eq!(
///     plugin_factory::unversioned("my_crate::points@2"),
///     "my_crate::points"
/// );
/// assert_eq!(
///     plugin_factory::unversioned("my_crate::points"),
///     "my_crate::points"
/// );
/// ```
pub fn unversioned(id: &str) -> &str {
    id.split_once(VERSION_SEPARATOR).map_or(id, |(id, _)| id)
}

/// The [`PluginType::ID`] of `plugin`, which may be behind a `dyn` of its axis.
pub fn id_of<P: Plugin + ?Sized>(plugin: &P) -> &'static str {
    __private::Erased::id(plugin)
}

/// `plugin`, which may be behind a `dyn` of its axis, as the plugin `T`, if it is one.
pub fn downcast_ref<T: PluginType, P: Plugin + ?Sized>(plugin: &P) -> Option<&T> {
    __private::Erased::as_any(plugin).downcast_ref()
}
