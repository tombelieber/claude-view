// crates/core/src/block_accumulator/interactions/mod.rs
//
// Historical InteractionBlock synthesiser — directory module root.
//
// Re-exports the public API so callers see the same surface as before.

mod synthesizer;

#[cfg(test)]
mod tests;

pub use synthesizer::synthesize_historical_interactions;
