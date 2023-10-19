use tokio::net::UdpSocket;
use std::io;


use serde::Deserialize;

/// This is what we're going to decode into.
#[derive(Debug, Deserialize)]
struct Config {
    plain: MyEnum,
    plain_table: MyEnum,
    tuple: MyEnum,
    #[serde(rename = "struct")]
    structv: MyEnum,
    newtype: MyEnum,
    my_enum: Vec<MyEnum>,
}

#[derive(Debug, Deserialize)]
enum MyEnum {
    Plain,
    Tuple(i64, bool),
    NewType(String),
    Struct { value: i64 },
}

fn main() {
    let toml_str = r#"
    plain = "Plain"
    plain_table = { Plain = {} }
    tuple = { Tuple = { 0 = 123, 1 = true } }
    struct = { Struct = { value = 123 } }
    newtype = { NewType = "value" }
    my_enum = [
        { Plain = {} },
        { Tuple = { 0 = 123, 1 = true } },
        { NewType = "value" },
        { Struct = { value = 123 } }
    ]"#;

    // toml::Table
    let decoded: Config = toml::from_str(toml_str).unwrap();
    println!("{:#?}", decoded);
}