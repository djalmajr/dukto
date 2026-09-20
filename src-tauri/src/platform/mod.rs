// Platform module — OS integration helpers.

#[cfg(target_os = "android")]
pub mod android_destination;

#[cfg(target_os = "ios")]
pub mod ios_power;
