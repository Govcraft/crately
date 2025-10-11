//! Retry policy configuration and backoff calculation for pipeline error recovery
//!
//! This module provides exponential backoff with jitter for retrying failed pipeline operations.
//! The retry policy is centralized and used by the RetryCoordinator actor to manage retry attempts
//! across all pipeline stages.

use std::time::Duration;

/// Retry policy configuration for pipeline error recovery
///
/// Implements exponential backoff with jitter to prevent thundering herd problems
/// when multiple crate downloads fail simultaneously.
///
/// # Default Configuration
///
/// - max_attempts: 3
/// - initial_delay_ms: 1000 (1 second)
/// - max_delay_ms: 30000 (30 seconds)
/// - backoff_multiplier: 2.0 (doubles each retry)
/// - jitter_factor: 0.1 (±10% randomization)
///
/// # Examples
///
/// ```
/// use crately::retry_policy::RetryPolicy;
///
/// let policy = RetryPolicy::default();
/// let delay = policy.calculate_delay(1); // First retry
/// assert!(delay.as_millis() >= 900 && delay.as_millis() <= 1100);
///
/// let delay = policy.calculate_delay(2); // Second retry
/// assert!(delay.as_millis() >= 1800 && delay.as_millis() <= 2200);
/// ```
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts before giving up
    pub max_attempts: u32,

    /// Initial delay in milliseconds before first retry
    pub initial_delay_ms: u64,

    /// Maximum delay in milliseconds (caps exponential growth)
    pub max_delay_ms: u64,

    /// Multiplier for exponential backoff (typically 2.0)
    pub backoff_multiplier: f64,

    /// Jitter factor for randomization (0.0 to 1.0)
    /// A value of 0.1 means ±10% randomization
    pub jitter_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

impl RetryPolicy {
    /// Creates a new retry policy with custom configuration
    ///
    /// # Arguments
    ///
    /// * `max_attempts` - Maximum number of retry attempts (must be > 0)
    /// * `initial_delay_ms` - Initial delay in milliseconds (must be > 0)
    /// * `max_delay_ms` - Maximum delay in milliseconds (must be >= initial_delay_ms)
    /// * `backoff_multiplier` - Exponential backoff multiplier (must be >= 1.0)
    /// * `jitter_factor` - Jitter randomization factor (must be 0.0 to 1.0)
    ///
    /// # Panics
    ///
    /// Panics if any validation constraint is violated.
    ///
    /// # Examples
    ///
    /// ```
    /// use crately::retry_policy::RetryPolicy;
    ///
    /// // Aggressive retry policy for critical operations
    /// let aggressive = RetryPolicy::new(5, 500, 60000, 2.0, 0.2);
    /// assert_eq!(aggressive.max_attempts, 5);
    ///
    /// // Conservative retry policy for rate-limited APIs
    /// let conservative = RetryPolicy::new(2, 5000, 120000, 3.0, 0.15);
    /// assert_eq!(conservative.initial_delay_ms, 5000);
    /// ```
    pub fn new(
        max_attempts: u32,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        backoff_multiplier: f64,
        jitter_factor: f64,
    ) -> Self {
        assert!(max_attempts > 0, "max_attempts must be greater than 0");
        assert!(initial_delay_ms > 0, "initial_delay_ms must be greater than 0");
        assert!(
            max_delay_ms >= initial_delay_ms,
            "max_delay_ms must be >= initial_delay_ms"
        );
        assert!(
            backoff_multiplier >= 1.0,
            "backoff_multiplier must be >= 1.0"
        );
        assert!(
            (0.0..=1.0).contains(&jitter_factor),
            "jitter_factor must be between 0.0 and 1.0"
        );

        Self {
            max_attempts,
            initial_delay_ms,
            max_delay_ms,
            backoff_multiplier,
            jitter_factor,
        }
    }

    /// Calculates the delay duration for a given retry attempt with exponential backoff and jitter
    ///
    /// # Arguments
    ///
    /// * `attempt` - The retry attempt number (1-indexed, first retry is attempt 1)
    ///
    /// # Returns
    ///
    /// A `Duration` representing the delay before the retry attempt, with jitter applied.
    /// The delay is capped at `max_delay_ms`.
    ///
    /// # Formula
    ///
    /// ```text
    /// base_delay = min(initial_delay * (backoff_multiplier ^ (attempt - 1)), max_delay)
    /// jitter_range = base_delay * jitter_factor
    /// final_delay = base_delay + random(-jitter_range, +jitter_range)
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use crately::retry_policy::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default();
    ///
    /// // First retry: ~1000ms ± 10%
    /// let delay1 = policy.calculate_delay(1);
    /// assert!(delay1.as_millis() >= 900 && delay1.as_millis() <= 1100);
    ///
    /// // Second retry: ~2000ms ± 10%
    /// let delay2 = policy.calculate_delay(2);
    /// assert!(delay2.as_millis() >= 1800 && delay2.as_millis() <= 2200);
    ///
    /// // Third retry: ~4000ms ± 10%
    /// let delay3 = policy.calculate_delay(3);
    /// assert!(delay3.as_millis() >= 3600 && delay3.as_millis() <= 4400);
    /// ```
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        use rand::Rng;

        // Calculate exponential backoff: initial_delay * (multiplier ^ (attempt - 1))
        let exponent = (attempt - 1) as f64;
        let base_delay_ms = self.initial_delay_ms as f64
            * self.backoff_multiplier.powf(exponent);

        // Cap at max_delay_ms
        let capped_delay_ms = base_delay_ms.min(self.max_delay_ms as f64);

        // Apply jitter: ±(delay * jitter_factor)
        let jitter_range = capped_delay_ms * self.jitter_factor;
        let mut rng = rand::rng();
        let jitter: f64 = rng.random_range(-jitter_range..=jitter_range);

        let final_delay_ms = (capped_delay_ms + jitter).max(0.0) as u64;

        Duration::from_millis(final_delay_ms)
    }

    /// Checks if another retry attempt should be made
    ///
    /// # Arguments
    ///
    /// * `current_attempt` - The current retry attempt number (1-indexed)
    ///
    /// # Returns
    ///
    /// `true` if another retry should be attempted, `false` if max attempts reached.
    ///
    /// # Examples
    ///
    /// ```
    /// use crately::retry_policy::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default();
    /// assert!(policy.should_retry(1));  // First retry allowed
    /// assert!(policy.should_retry(2));  // Second retry allowed
    /// assert!(policy.should_retry(3));  // Third retry allowed
    /// assert!(!policy.should_retry(4)); // Fourth retry exceeds max_attempts
    /// ```
    pub fn should_retry(&self, current_attempt: u32) -> bool {
        current_attempt <= self.max_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.initial_delay_ms, 1000);
        assert_eq!(policy.max_delay_ms, 30000);
        assert_eq!(policy.backoff_multiplier, 2.0);
        assert_eq!(policy.jitter_factor, 0.1);
    }

    #[test]
    fn test_new_policy() {
        let policy = RetryPolicy::new(5, 500, 60000, 2.5, 0.15);
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.initial_delay_ms, 500);
        assert_eq!(policy.max_delay_ms, 60000);
        assert_eq!(policy.backoff_multiplier, 2.5);
        assert_eq!(policy.jitter_factor, 0.15);
    }

    #[test]
    #[should_panic(expected = "max_attempts must be greater than 0")]
    fn test_new_policy_zero_attempts() {
        RetryPolicy::new(0, 1000, 30000, 2.0, 0.1);
    }

    #[test]
    #[should_panic(expected = "initial_delay_ms must be greater than 0")]
    fn test_new_policy_zero_initial_delay() {
        RetryPolicy::new(3, 0, 30000, 2.0, 0.1);
    }

    #[test]
    #[should_panic(expected = "max_delay_ms must be >= initial_delay_ms")]
    fn test_new_policy_invalid_max_delay() {
        RetryPolicy::new(3, 5000, 1000, 2.0, 0.1);
    }

    #[test]
    #[should_panic(expected = "backoff_multiplier must be >= 1.0")]
    fn test_new_policy_invalid_multiplier() {
        RetryPolicy::new(3, 1000, 30000, 0.5, 0.1);
    }

    #[test]
    #[should_panic(expected = "jitter_factor must be between 0.0 and 1.0")]
    fn test_new_policy_invalid_jitter() {
        RetryPolicy::new(3, 1000, 30000, 2.0, 1.5);
    }

    #[test]
    fn test_calculate_delay_exponential_growth() {
        let policy = RetryPolicy {
            max_attempts: 5,
            initial_delay_ms: 1000,
            max_delay_ms: 100000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0, // No jitter for predictable testing
        };

        let delay1 = policy.calculate_delay(1);
        assert_eq!(delay1.as_millis(), 1000); // 1000 * 2^0

        let delay2 = policy.calculate_delay(2);
        assert_eq!(delay2.as_millis(), 2000); // 1000 * 2^1

        let delay3 = policy.calculate_delay(3);
        assert_eq!(delay3.as_millis(), 4000); // 1000 * 2^2

        let delay4 = policy.calculate_delay(4);
        assert_eq!(delay4.as_millis(), 8000); // 1000 * 2^3
    }

    #[test]
    fn test_calculate_delay_caps_at_max() {
        let policy = RetryPolicy {
            max_attempts: 10,
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
        };

        let delay5 = policy.calculate_delay(5);
        assert_eq!(delay5.as_millis(), 5000); // Capped at max_delay_ms

        let delay10 = policy.calculate_delay(10);
        assert_eq!(delay10.as_millis(), 5000); // Still capped
    }

    #[test]
    fn test_calculate_delay_with_jitter() {
        let policy = RetryPolicy::default(); // jitter_factor = 0.1

        // Run multiple times to verify jitter is applied
        let mut delays = Vec::new();
        for _ in 0..10 {
            let delay = policy.calculate_delay(1);
            delays.push(delay.as_millis());
        }

        // All delays should be within ±10% of 1000ms
        for delay_ms in delays.iter() {
            assert!(*delay_ms >= 900 && *delay_ms <= 1100);
        }

        // At least some delays should be different (probabilistic test)
        let unique_delays: std::collections::HashSet<_> = delays.into_iter().collect();
        assert!(unique_delays.len() > 1, "Jitter should produce varied delays");
    }

    #[test]
    fn test_should_retry() {
        let policy = RetryPolicy::default(); // max_attempts = 3

        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(policy.should_retry(3));
        assert!(!policy.should_retry(4));
        assert!(!policy.should_retry(5));
    }

    #[test]
    fn test_should_retry_zero_attempts() {
        let policy = RetryPolicy::new(1, 1000, 30000, 2.0, 0.1);

        assert!(policy.should_retry(1));
        assert!(!policy.should_retry(2));
    }

    #[test]
    fn test_policy_clone() {
        let policy1 = RetryPolicy::default();
        let policy2 = policy1.clone();

        assert_eq!(policy1.max_attempts, policy2.max_attempts);
        assert_eq!(policy1.initial_delay_ms, policy2.initial_delay_ms);
        assert_eq!(policy1.max_delay_ms, policy2.max_delay_ms);
    }

    #[test]
    fn test_real_world_scenario() {
        // Simulate rate limit retry scenario
        let rate_limit_policy = RetryPolicy::new(3, 5000, 60000, 2.0, 0.1);

        let delay1 = rate_limit_policy.calculate_delay(1);
        // ~5000ms ± 10%
        assert!(delay1.as_millis() >= 4500 && delay1.as_millis() <= 5500);

        let delay2 = rate_limit_policy.calculate_delay(2);
        // ~10000ms ± 10%
        assert!(delay2.as_millis() >= 9000 && delay2.as_millis() <= 11000);

        let delay3 = rate_limit_policy.calculate_delay(3);
        // ~20000ms ± 10%
        assert!(delay3.as_millis() >= 18000 && delay3.as_millis() <= 22000);

        assert!(rate_limit_policy.should_retry(3));
        assert!(!rate_limit_policy.should_retry(4));
    }
}
