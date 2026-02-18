//! mindtask CLI binary.

mod cli;

fn main() -> anyhow::Result<()> {
    cli::run()
}
