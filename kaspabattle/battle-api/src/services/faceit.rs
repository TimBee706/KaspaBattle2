#[allow(dead_code)]
pub async fn create_faceit_match(
    player_a: &str,
    player_b: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(format!("faceit_mk_{}_{}", player_a, player_b))
}
