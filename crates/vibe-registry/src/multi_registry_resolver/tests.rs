//! Tests for the resolver's own surface — the mirror-chain filter and
//! sort exposed via [`MultiRegistryResolver::mirrors_for`].

specmark::scope!("spec://vibevm/modules/vibe-registry/PROP-002#registry-model");

use super::*;
use tempfile::tempdir;

use crate::multi_registry_resolver::test_support::*;

#[test]
fn mirrors_for_filters_and_sorts() {
    let cache = tempdir().unwrap();
    let fake = Arc::new(FakeBackend::default());

    let mirrors = vec![
        MirrorSection {
            of: "vibespecs".to_string(),
            url: "https://a".to_string(),
            priority: 2,
        },
        MirrorSection {
            of: "vibespecs".to_string(),
            url: "https://b".to_string(),
            priority: 1,
        },
        MirrorSection {
            of: "*".to_string(),
            url: "https://catchall".to_string(),
            priority: 99,
        },
        MirrorSection {
            of: "other".to_string(),
            url: "https://unrelated".to_string(),
            priority: 0,
        },
    ];
    let r = build_resolver(
        cache.path(),
        vec![registry_section("vibespecs", "git@host:org")],
        mirrors,
        vec![],
        fake,
    );

    let m = r.mirrors_for("vibespecs");
    assert_eq!(m.len(), 3);
    assert_eq!(m[0].url, "https://b");
    assert_eq!(m[1].url, "https://a");
    assert_eq!(m[2].url, "https://catchall");
}
