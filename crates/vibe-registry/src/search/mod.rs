//! `vibe search`'s registry-side domain — the search machinery the CLI
//! used to carry inline (CONVERT-PLAN v0.1 §4.1). The index-consuming
//! client (`IndexClient`, `SearchResults`) already lives beside this in
//! [`index_client`](crate::index_client); this family is its degraded-mode
//! companion: the result [`cache`] and the full-scan fallback
//! ([`full_scan`]); the scoring lands next, so the whole search surface
//! has one home, not two.

pub mod cache;
pub mod full_scan;
pub mod query;
