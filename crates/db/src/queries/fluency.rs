// crates/db/src/queries/fluency.rs
//! Database bridge for computing the current fluency score from session facets.

use claude_view_core::fluency_score::{compute_fluency_score, FluencyScore, ScoreInput};

impl crate::Database {
    /// Compute the current fluency score from aggregate facet data.
    ///
    /// Queries `session_facets` for achievement, friction, and satisfaction
    /// metrics, then feeds them into the pure math `compute_fluency_score`.
    /// Cost efficiency and consistency are placeholders (0.5) until we
    /// have real data sources for them.
    pub async fn compute_current_fluency_score(&self) -> sqlx::Result<FluencyScore> {
        let stats = self.get_facet_aggregate_stats().await?;

        if stats.total_with_facets == 0 {
            return Ok(FluencyScore {
                score: 0,
                achievement_rate: 0.0,
                friction_rate: 0.0,
                cost_efficiency: 0.0,
                satisfaction_trend: 0.0,
                consistency: 0.0,
                sessions_analyzed: 0,
            });
        }

        // achievement_rate from DB is 0-100 (percentage); normalize to 0.0-1.0
        let achievement_rate = stats.achievement_rate / 100.0;
        let friction_rate =
            stats.friction_session_count as f64 / stats.total_with_facets as f64;
        let satisfaction_trend =
            stats.satisfied_or_above_count as f64 / stats.total_with_facets as f64;

        // Placeholders until we have real cost + consistency data
        let cost_efficiency = 0.5;
        let consistency = 0.5;

        let input = ScoreInput {
            achievement_rate,
            friction_rate,
            cost_efficiency,
            satisfaction_trend,
            consistency,
        };

        let score = compute_fluency_score(&input);

        Ok(FluencyScore {
            score,
            achievement_rate,
            friction_rate,
            cost_efficiency,
            satisfaction_trend,
            consistency,
            sessions_analyzed: stats.total_with_facets,
        })
    }
}
