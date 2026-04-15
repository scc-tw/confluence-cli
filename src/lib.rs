pub mod api;
pub mod app;
pub mod cli;
pub mod config;
pub mod convert;
pub mod domain;
pub mod render;
pub mod support;

pub fn run() -> support::Result<()> {
    cli::run()
}

pub fn run_from<I, T>(args: I) -> support::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    cli::run_from(args)
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_runs_helpfully() {
        assert!(super::run_from(["confluence", "--help"]).is_ok());
    }
}
