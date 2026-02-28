use uuid::Uuid;

#[allow(dead_code)]
pub async fn create_deposit_tx(
    _match_id: Uuid,
    _kaspa_address: &str,
    _amount: u64,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok("0xunsigned_mock_payload".to_string())
}
