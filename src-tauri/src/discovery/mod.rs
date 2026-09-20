#[cfg(not(target_os = "ios"))]
pub mod mdns;
#[cfg(target_os = "ios")]
#[path = "mdns_ios.rs"]
pub mod mdns;
pub mod types;
