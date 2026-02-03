// PPLNS Payment Logic Validation Module for DMPool
// Validates the correctness of PPLNS payout calculations

use anyhow::Result;
use chrono::{DateTime, Utc};
use p2poolv2_lib::accounting::simple_pplns::SimplePplnsShare;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// PPLNS payout calculation result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PayoutCalculation {
    /// Miner address
    pub address: String,
    /// Worker name
    pub worker: String,
    /// Share count in PPLNS window
    pub share_count: u64,
    /// Total difficulty of shares
    pub total_difficulty: u64,
    /// Proportional payout (satoshi)
    pub payout_satoshis: u64,
    /// PPLNS window size (last N shares)
    pub pplns_window_size: u64,
    /// Block reward (satoshi)
    pub block_reward_satoshis: u64,
    /// Pool fee/deduction (satoshi)
    pub pool_fee_satoshis: u64,
    /// Final payout amount
    pub final_payout_satoshis: u64,
}

/// PPLNS validation result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PplnsValidationResult {
    /// Overall validation status
    pub valid: bool,
    /// Total shares checked
    pub total_shares: u64,
    /// Unique miners found
    pub unique_miners: u64,
    /// Payout calculations
    pub payouts: Vec<PayoutCalculation>,
    /// Total payout amount
    pub total_payout_satoshis: u64,
    /// Validation errors
    pub errors: Vec<String>,
    /// Warnings
    pub warnings: Vec<String>,
    /// Timestamp of validation
    pub validated_at: DateTime<Utc>,
}

/// PPLNS payment simulator for testing
pub struct PplnsSimulator {
    /// Block reward in satoshis (for mainnet, this is variable)
    block_reward_satoshis: u64,
    /// Pool fee percentage (basis points: 100 = 1%)
    pool_fee_bps: u16,
    /// PPLNS window time window (days)
    pplns_window_days: u64,
}

impl PplnsSimulator {
    /// Create a new PPLNS simulator
    pub fn new(block_reward_satoshis: u64, pool_fee_bps: u16, pplns_window_days: u64) -> Self {
        Self {
            block_reward_satoshis,
            pool_fee_bps,
            pplns_window_days,
        }
    }

    /// Default simulator (using mainnet values)
    pub fn default() -> Self {
        Self::new(
            100_000_000, // 1 BTC in satoshis (actual block reward + fees)
            0,           // No pool fee by default
            7,           // 7-day PPLNS window
        )
    }

    /// Calculate payout for a single miner based on their shares
    pub fn calculate_payout(
        &self,
        shares: &[SimplePplnsShare],
        miner_address: &str,
    ) -> Option<PayoutCalculation> {
        if shares.is_empty() {
            return None;
        }

        // Filter shares for this miner
        let miner_shares: Vec<_> = shares
            .iter()
            .filter(|s| s.btcaddress.as_ref().map_or(false, |addr| addr == miner_address))
            .collect();

        if miner_shares.is_empty() {
            return None;
        }

        // Calculate total difficulty of miner's shares
        let total_difficulty: u64 = miner_shares.iter().map(|s| s.difficulty).sum();

        // Calculate total difficulty of all shares in PPLNS window
        let window_difficulty: u64 = shares.iter().map(|s| s.difficulty).sum();

        if window_difficulty == 0 {
            return None;
        }

        // Calculate proportional payout using u128 to prevent overflow
        // (block_reward_satoshis * total_difficulty) could be very large
        let proportional_payout: u128 = (self.block_reward_satoshis as u128)
            * (total_difficulty as u128)
            / (window_difficulty as u128);

        // Calculate pool fee using u128 to prevent overflow
        let pool_fee: u128 = (proportional_payout
            * (self.pool_fee_bps as u128))
            / 10000u128;

        // Final payout (ensure no negative values)
        let final_payout = proportional_payout
            .saturating_sub(pool_fee)
            .min(u64::MAX as u128) as u64;

        // Convert pool_fee back to u64 for storage
        let pool_fee_u64 = pool_fee.min(u64::MAX as u128) as u64;

        Some(PayoutCalculation {
            address: miner_address.to_string(),
            worker: miner_shares
                .first()
                .and_then(|s| s.workername.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            share_count: miner_shares.len() as u64,
            total_difficulty,
            payout_satoshis: proportional_payout.min(u64::MAX as u128) as u64,
            pplns_window_size: shares.len() as u64,
            block_reward_satoshis: self.block_reward_satoshis,
            pool_fee_satoshis: pool_fee_u64,
            final_payout_satoshis: final_payout,
        })
    }

    /// Simulate payouts for all miners in a share set
    pub fn simulate_payouts(&self, shares: &[SimplePplnsShare]) -> PplnsValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut payouts = Vec::new();
        let mut total_payout = 0u64;
        let mut unique_miners: HashSet<String> = HashSet::new();

        // Get unique miner addresses
        for share in shares {
            if let Some(ref addr) = share.btcaddress {
                unique_miners.insert(addr.clone());
            }
        }

        // Calculate payout for each miner
        for miner_addr in unique_miners.iter() {
            if let Some(payout) = self.calculate_payout(shares, miner_addr) {
                total_payout += payout.final_payout_satoshis;
                payouts.push(payout);
            }
        }

        // Validate calculations
        let _total_difficulty: u64 = shares.iter().map(|s| s.difficulty).sum();
        let expected_total_payout = self.block_reward_satoshis.saturating_sub(
            (self.block_reward_satoshis * self.pool_fee_bps as u64) / 10000
        );

        // Check if payouts exceed block reward
        if total_payout > expected_total_payout {
            errors.push(format!(
                "Total payouts ({}) exceed available reward ({})",
                total_payout, expected_total_payout
            ));
        }

        // Check for negative payouts
        for payout in &payouts {
            if payout.final_payout_satoshis == 0 && payout.share_count > 0 {
                warnings.push(format!(
                    "Miner {} has shares but zero payout (difficulty too low?)",
                    payout.address
                ));
            }
        }

        PplnsValidationResult {
            valid: errors.is_empty(),
            total_shares: shares.len() as u64,
            unique_miners: unique_miners.len() as u64,
            payouts,
            total_payout_satoshis: total_payout,
            errors,
            warnings,
            validated_at: Utc::now(),
        }
    }

    /// Validate share difficulty bounds
    pub fn validate_difficulty_bounds(&self, shares: &[SimplePplnsShare]) -> Result<(), String> {
        if shares.is_empty() {
            return Err("No shares to validate".to_string());
        }

        let min_diff = shares.iter().map(|s| s.difficulty).min().unwrap_or(0);
        let max_diff = shares.iter().map(|s| s.difficulty).max().unwrap_or(0);

        if min_diff == 0 {
            return Err("Found share with zero difficulty".to_string());
        }

        if max_diff / min_diff > 1000 {
            return Err(format!(
                "Difficulty range too large: min={}, max={} (ratio: {})",
                min_diff,
                max_diff,
                max_diff / min_diff
            ));
        }

        Ok(())
    }

    /// Validate PPLNS window size
    pub fn validate_window_size(&self, shares: &[SimplePplnsShare], expected_window_days: u64) -> Result<(), String> {
        if shares.is_empty() {
            return Ok(());
        }

        let oldest_share = shares.iter().min_by_key(|a| a.n_time);
        let newest_share = shares.iter().max_by_key(|a| a.n_time);

        if let (Some(oldest), Some(newest)) = (oldest_share, newest_share) {
            let time_span = newest.n_time.saturating_sub(oldest.n_time);
            let time_span_days = time_span / 86400; // Convert seconds to days

            if time_span_days > expected_window_days * 2 {
                return Err(format!(
                    "PPLNS window spans {} days, expected around {} days",
                    time_span_days, expected_window_days
                ));
            }
        }

        Ok(())
    }
}

/// PPLNS validation test scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationScenario {
    pub name: String,
    pub description: String,
    pub expected_result: String,
}

impl PplnsSimulator {
    /// Run standard validation scenarios
    pub async fn run_scenarios(&self, shares: &[SimplePplnsShare]) -> Vec<ScenarioResult> {
        let mut results = Vec::new();

        // Scenario 1: Normal operation
        results.push(self.test_scenario("Normal payout calculation", shares));

        // Scenario 2: Empty shares
        results.push(self.test_scenario("Empty shares", &[]));

        // TODO: Add more scenarios

        results
    }

    fn test_scenario(&self, name: &str, shares: &[SimplePplnsShare]) -> ScenarioResult {
        let validation = self.simulate_payouts(shares);

        ScenarioResult {
            name: name.to_string(),
            passed: validation.valid,
            result: if validation.valid {
                "PASS".to_string()
            } else {
                format!("FAIL: {}", validation.errors.join("; "))
            },
            details: validation,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub passed: bool,
    pub result: String,
    pub details: PplnsValidationResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_share(address: &str, difficulty: u64, time: u64) -> SimplePplnsShare {
        SimplePplnsShare {
            btcaddress: Some(address.to_string()),
            workername: Some("test-worker".to_string()),
            user_id: 1,
            difficulty,
            n_time: time,
            job_id: format!("job-{}", time),
            extranonce2: "00000001".to_string(),
            nonce: format!("{:08x}", time),
        }
    }

    #[test]
    fn test_payout_calculation() {
        let simulator = PplnsSimulator::new(
            100_000_000, // 1 BTC
            100,        // 1% pool fee
            7,           // 7-day window
        );

        // Create test shares
        let shares = vec![
            create_test_share("bc1qtest1", 1000, 1000),
            create_test_share("bc1qtest1", 2000, 2000),
            create_test_share("bc1qtest2", 1500, 3000),
            create_test_share("bc1qtest3", 500, 4000),
        ];

        let validation = simulator.simulate_payouts(&shares);

        assert!(validation.valid);
        assert_eq!(validation.unique_miners, 3);

        // Check bc1qtest1 has 3000 difficulty out of 5000 total
        let test1_payout = validation.payouts.iter()
            .find(|p| p.address == "bc1qtest1")
            .unwrap();
        assert_eq!(test1_payout.total_difficulty, 3000);
        assert_eq!(test1_payout.share_count, 2);

        // Calculate expected payout
        // Total: 100M satoshis (1 BTC)
        // bc1qtest1: 100M * 3000/5000 = 60M satoshis
        // Pool fee: 60M * 100/10000 = 0.6M satoshis
        // Final: 60M - 0.6M = 59.4M satoshis
        assert_eq!(test1_payout.final_payout_satoshis, 59400000);
    }

    #[test]
    fn test_difficulty_validation() {
        let simulator = PplnsSimulator::default();

        // Valid shares
        let shares = vec![
            create_test_share("bc1qtest1", 1000, 1000),
            create_test_share("bc1qtest1", 2000, 2000),
        ];

        assert!(simulator.validate_difficulty_bounds(&shares).is_ok());

        // Invalid: zero difficulty
        let invalid_shares = vec![
            create_test_share("bc1qtest1", 0, 1000),
        ];
        assert!(simulator.validate_difficulty_bounds(&invalid_shares).is_err());

        // Invalid: too wide range
        let wide_range_shares = vec![
            create_test_share("bc1qtest1", 1, 1000),
            create_test_share("bc1qtest1", 10000, 2000),
        ];
        assert!(simulator.validate_difficulty_bounds(&wide_range_shares).is_err());
    }

    #[test]
    fn test_window_validation() {
        let simulator = PplnsSimulator::default();

        // Valid window (7 days)
        let now = Utc::now().timestamp() as u64;
        let shares = vec![
            create_test_share("bc1qtest1", 1000, now - 86400 * 6), // 6 days ago
            create_test_share("bc1qtest1", 1000, now),              // now
        ];

        assert!(simulator.validate_window_size(&shares, 7).is_ok());

        // Invalid window (too wide)
        let wide_shares = vec![
            create_test_share("bc1qtest1", 1000, now - 86400 * 20), // 20 days ago
            create_test_share("bc1qtest1", 1000, now),              // now
        ];

        assert!(simulator.validate_window_size(&wide_shares, 7).is_err());
    }
}
