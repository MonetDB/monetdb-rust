use std::env;

use anyhow::{bail, Result as AResult};
use log::info;

use monetdb::{conn::Connection, parms::Parameters};

fn main() -> AResult<()> {
    configure_logging()?;

    let mut parms = Parameters::default()
        .with_user("monetdb")?
        .with_password("monetdb")?;
    let Some(url) = env::args().skip(1).next() else {
        bail!("Usage: connect URL");
    };
    parms.apply_url(&url)?;
    let _conn = Connection::new(parms)?;
    info!("connected.");
    Ok(())
}

fn configure_logging() -> AResult<()> {
    let mut builder = simplelog::ConfigBuilder::new();
    builder.set_thread_level(log::LevelFilter::Off);
    let _ = builder.set_time_offset_to_local();
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Trace,
        builder.build(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )?;
    Ok(())
}
