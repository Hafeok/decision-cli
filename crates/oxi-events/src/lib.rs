//! oxi-events — graph-native event substrate for Oxigraph.
//!
//! Stable Dependency Principle: this crate speaks only of mutations,
//! subscriptions, events, and delivery. It MUST NOT depend on
//! decision-cli or reference DDD concepts (roles, bundles, sessions,
//! policies, autonomy levels). See ADR-001.
