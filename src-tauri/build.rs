fn main() {
    #[cfg(feature = "app-common")]
    tauri_build::build();
}
