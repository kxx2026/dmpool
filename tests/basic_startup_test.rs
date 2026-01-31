use anyhow::Result;

#[tokio::test]
async fn test_config_loading() -> Result<()> {
    use p2poolv2_lib::config::Config;
    
    let config_content = r#"
[store]
path = "/tmp/test_store.db"
background_task_frequency_hours = 24
pplns_ttl_days = 1

[stratum]
hostname = "127.0.0.1"
port = 3333
start_difficulty = 1
minimum_difficulty = 1
bootstrap_address = "bcrt1qce93hy5rhg02s6aeu7mfdvxg76x66pqqtrvzs3"
zmqpubhashblock = "tcp://127.0.0.1:28334"
network = "signet"
pool_signature = "test_pool"
version_mask = "1fffe000"
difficulty_multiplier = 1.0
ignore_difficulty = true

[bitcoinrpc]
url = "http://127.0.0.1:18443"
username = "bitcoin"
password = "bitcoin"

[logging]
level = "info"

[api]
hostname = "127.0.0.1"
port = 46884
"#;

    let config_path = "/tmp/test_config.toml";
    tokio::fs::write(config_path, config_content).await?;

    let result = Config::load(config_path);
    
    match &result {
        Ok(config) => {
            assert_eq!(config.stratum.port, 3333);
            assert_eq!(config.api.port, 46884);
        }
        Err(e) => {
            panic!("Config loading failed: {}", e);
        }
    }
    
    tokio::fs::remove_file(config_path).await.ok();
    
    Ok(())
}

#[tokio::test]
async fn test_database_initialization() -> Result<()> {
    use p2poolv2_lib::store::Store;
    
    let db_path = "/tmp/test_db_init.db";
    
    tokio::fs::remove_file(db_path).await.ok();
    
    let store_result = Store::new(db_path.to_string(), false);
    assert!(store_result.is_ok(), "Database should initialize successfully");
    
    tokio::fs::remove_file(db_path).await.ok();
    
    Ok(())
}

#[tokio::test]
async fn test_genesis_block_creation() {
    use p2poolv2_lib::shares::share_block::ShareBlock;
    use bitcoin::Network;
    
    let genesis = ShareBlock::build_genesis_for_network(Network::Signet);
    assert_eq!(genesis.transactions.len(), 1);
}

#[tokio::test]
async fn test_network_types() {
    use bitcoin::Network;
    
    assert_eq!(Network::Bitcoin.to_string(), "bitcoin");
    assert_eq!(Network::Signet.to_string(), "signet");
    assert_eq!(Network::Testnet.to_string(), "testnet");
}
