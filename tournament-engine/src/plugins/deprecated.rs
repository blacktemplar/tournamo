//! Earlier versions of plugins, which competitions saved with them keep playing, see
//! [`PluginType::VERSION`](plugin_factory::PluginType::VERSION). Each is a frozen copy of the
//! plugin as it was, sharing nothing with its later versions but what the engine provides every
//! plugin, and the types a saved competition holds (annotations, metrics, the bracket); it is
//! never changed, only dropped once old enough.

pub mod double_elimination_v1;

pub use double_elimination_v1::DoubleEliminationV1;
