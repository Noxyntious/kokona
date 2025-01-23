pub mod versioninfo {
    const fn get_version() -> &'static str {
        #[cfg(debug_assertions)]
        {
            concat!(env!("CARGO_PKG_VERSION"), "-dev")
        }
        #[cfg(not(debug_assertions))]
        {
            env!("CARGO_PKG_VERSION")
        }
    }

    pub const VERSION: &str = get_version();
}
