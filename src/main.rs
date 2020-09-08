use std::collections::HashMap;
use std::convert::TryInto;
use std::error::Error;
use std::fs;
use std::net::IpAddr;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use native_tls::TlsConnector;
use postgres::types::{FromSql, Type, WrongType};
use postgres::Client;
use postgres::NoTls;
use postgres::Row;
use postgres_native_tls::MakeTlsConnector;
use serde_json::json;
use serde_json::Map as JsonMap;
use serde_json::Value;

mod config;

fn main() -> Result<()> {
    let config = fs::read("queries.yml")?;
    let config: config::RootConfig = serde_yaml::from_slice(&config)?;
    for server in config.servers {
        // let connector = TlsConnector::builder().build()?;
        // let connector = MakeTlsConnector::new(connector);
        let mut client = Client::connect(&server.connect.url, NoTls)?;
        for query in server.queries {
            let submitted = Utc::now();
            let results = client.query(query.sql.as_str(), &[])?;
            let mut lines = Vec::with_capacity(results.len());
            for result in results {
                let columns = result.columns();
                let mut line = JsonMap::with_capacity(columns.len());
                for col in 0..result.len() {
                    line.insert(columns[col].name().to_string(), map_type(&result, col)?);
                }
                lines.push(line);
            }
            println!(
                "{}",
                json!({
                    "server": server.name,
                    "query": query.name,
                    "now": submitted,
                    "results": lines,
                })
            );
        }
    }
    Ok(())
}

// https://docs.rs/postgres/0.17.5/postgres/types/trait.FromSql.html#types
fn map_type(result: &Row, col: usize) -> Result<Value> {
    macro_rules! try_get_display {
        ($target:ty) => {
            match result.try_get::<_, Option<$target>>(col) {
                Ok(Some(val)) => return Ok(json!(format!("{}", val))),
                Ok(None) => return Ok(json!(null)),
                Err(e) => handle_type_error(e)?,
            };
        };
    }

    macro_rules! try_get_json {
        ($target:ty) => {
            match result.try_get::<_, Option<$target>>(col) {
                Ok(Some(val)) => return Ok(json!(val)),
                Ok(None) => return Ok(json!(null)),
                Err(e) => handle_type_error(e)?,
            };
        };
    }

    try_get_display!(&str);
    try_get_display!(i64);

    try_get_json!(i32);
    try_get_json!(u32);
    try_get_json!(i16);
    try_get_json!(i8);
    try_get_json!(bool);
    try_get_json!(f64);
    try_get_json!(f32);
    try_get_json!(DateTime<Utc>);
    try_get_json!(NaiveDateTime);
    try_get_json!(IpAddr);

    match result.try_get::<_, Option<Value>>(col) {
        Ok(Some(val)) => return Ok(val),
        Ok(None) => return Ok(json!(null)),
        Err(e) => handle_type_error(e)?,
    };

    match result.try_get::<_, Option<Xid>>(col) {
        Ok(Some(val)) => return Ok(json!(val.0)),
        Ok(None) => return Ok(json!(null)),
        Err(e) => handle_type_error(e)?,
    };

    match result.try_get::<_, Option<RawBytes>>(col) {
        Ok(Some(val)) => bail!("unknown type: {:?}", val),
        Ok(None) => return Ok(json!(null)),
        Err(e) => handle_type_error(e)?,
    };

    Err(anyhow!("unmappable type {:?}", result.columns()[col]))
}

#[derive(Copy, Clone, Debug)]
struct Xid(u32);

impl<'f> FromSql<'f> for Xid {
    fn from_sql(_: &Type, raw: &[u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        // TODO: native?
        Ok(Xid(u32::from_ne_bytes(raw.try_into()?)))
    }

    fn accepts(ty: &Type) -> bool {
        match *ty {
            Type::XID => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
struct RawBytes {
    type_: Type,
    inner: Vec<u8>,
}

impl<'f> FromSql<'f> for RawBytes {
    fn from_sql(type_: &Type, raw: &[u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        Ok(RawBytes {
            type_: type_.clone(),
            inner: raw.to_vec(),
        })
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}

fn handle_type_error(e: postgres::Error) -> Result<()> {
    if wrong_type(&e) {
        Ok(())
    } else {
        Err(e)?
    }
}

fn wrong_type(e: &postgres::Error) -> bool {
    e.source()
        .and_then(|e| e.downcast_ref::<WrongType>())
        .is_some()
}
