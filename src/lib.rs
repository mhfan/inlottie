
pub mod core;
pub mod backend;

#[cfg(feature = "rive-rs")] pub mod rive_nvg;
#[cfg(feature = "vello")] pub mod vello_svg;

#[path = "rive/schema.rs"] pub mod rive_schema;
