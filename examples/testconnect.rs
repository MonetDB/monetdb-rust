use std::env;

use anyhow::{bail, Result as AResult};
use log::info;

use monetdb::{conn::Connection, cursor::Cursor, parms::Parameters};

const DEFAULT_QUERY: &str = r##"
DROP TABLE IF EXISTS foo;
CREATE TABLE foo(i INT, t VARCHAR(10));
SELECT value FROM sys.generate_series(0,5);
INSERT INTO foo VALUES (1, 'one'), (42, 'forty-two'), (-1, R'a\"b');
SELECT * FROM foo
"##;

fn main() -> AResult<()> {
    configure_logging()?;

    let mut arg_iter = env::args().skip(1);
    let Some(url) = arg_iter.next() else {
        bail!("Usage: connect URL");
    };

    let mut parms = Parameters::default()
        .with_user("monetdb")?
        .with_password("monetdb")?;
    parms.apply_url(&url)?;
    let conn = Connection::new(parms)?;
    info!("connected.");
    let mut cursor: Cursor = conn.cursor();

    let mut queries: Vec<String> = arg_iter.collect();
    if queries.is_empty() {
        queries.push(DEFAULT_QUERY.trim().to_string());
    }

    for query in queries {
        println!();
        println!("================================================================");
        println!("{query}");
        println!("================================================================");
        let resp = cursor.execute(&query)?;
        println!("{resp}");
        println!("----------------------------------------------------------------")
    }

    conn.close();
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
