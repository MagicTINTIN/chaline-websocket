use serde_json::{Result, Value};
use std::fs;

pub fn load_config() -> Result<()> {
    // Some JSON input data as a &str. Maybe this comes from the user.
    // let _data = r#"
    //     {
    //         "name": "John Doe",
    //         "age": 43,
    //         "phones": [
    //             "+44 1234567",
    //             "+44 2345678"
    //         ]
    //     }"#;

    let json_data = fs::read_to_string("data.json").unwrap();

    // Parse the string of data into serde_json::Value.
    let v: Value = serde_json::from_str(json_data.as_str())?;

    // Access parts of the data by indexing with square brackets.
    println!("Please call {} at the number {}", v["name"], v["phones"][0]);

    Ok(())
}
