#[cfg(feature = "dev")]
mod dev;

#[cfg(not(feature = "dev"))]
mod headless;

pub(crate) struct ServerPlugin;