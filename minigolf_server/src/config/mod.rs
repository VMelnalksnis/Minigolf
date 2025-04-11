#[cfg(feature = "ui")]
mod dev;

#[cfg(not(feature = "ui"))]
mod headless;

pub(crate) struct ServerPlugin;