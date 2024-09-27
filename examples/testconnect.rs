use std::env;
use std::fmt::Write;

use anyhow::{bail, Result as AResult};
use log::info;

use monetdb::{conn::Connection, cursor::Cursor, parms::Parameters};

const DEFAULT_QUERY: &str = r##"
DROP TABLE IF EXISTS foo;
CREATE TABLE foo(i INT, t VARCHAR(10));
SELECT value FROM sys.generate_series(0,5);
INSERT INTO foo VALUES (1, 'one'), (42, 'forty-two'), (-1, R'a\"b');
SELECT * FROM foo;
SELECT * FROM sys.unclosed_result_sets();
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
        cursor.execute(&query)?;
        loop {
            if let Some(row_count) = cursor.affected_rows() {
                if cursor.has_result_set() {
                    let md = cursor.metadata().to_vec();
                    let ncols = md.len();
                    println!("RESULT, {row_count} rows, {ncols} cols: {md:?}");
                    let mut i = 0;
                    let mut buf = String::new();
                    while cursor.next_row()? {
                        i += 1;
                        println!("  - ROW {i}/{row_count}:");
                        for (i, col) in md.iter().enumerate() {
                            let name = col.name();
                            let sql_type = col.sql_type();
                            buf.clear();
                            write!(buf, "{name} [{sql_type}]").unwrap();
                            let value = cursor.get_str(i)?;
                            if let Some(s) = value {
                                println!("      {buf:26} = {s}");
                            } else {
                                println!("      {buf:26} is NULL");
                            }
                        }
                    }
                    // let rs = cursor.temporary_get_result_set()?.unwrap().trim_end();
                    // println!("{rs}")
                } else {
                    println!("OK, {row_count} affected rows");
                }
            } else {
                println!("OK");
            }
            if !cursor.next_reply()? {
                break;
            }
        }
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
