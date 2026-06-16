fn main() -> Result<(), Box<dyn std::error::Error>> {
    let contract = rts_protocol::protocol_contract();
    println!("{}", serde_json::to_string_pretty(&contract)?);
    Ok(())
}
