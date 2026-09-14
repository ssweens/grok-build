//! `FEATURES` is the source of truth and the operator tables are hand-maintained mirrors with no compile-time check of their own.
//! This test is that check.

use std::path::Path;
use xai_grok_shell::agent::config::FEATURES;

const ENTERPRISE_PATH: &str = "../docs/internal/25-enterprise.md";
const ENV_VARS_PATH: &str = "../docs/internal/22-environment-variables.md";

#[test]
fn every_registered_feature_reaches_the_operator() {
    // The operator tables live in the monorepo's docs/internal, which the
    // OSS sync does not include; skip the check where they are absent.
    let Some(enterprise) = read_optional(ENTERPRISE_PATH) else {
        eprintln!("skip: {ENTERPRISE_PATH} not present in this tree");
        return;
    };
    let Some(env_vars) = read_optional(ENV_VARS_PATH) else {
        eprintln!("skip: {ENV_VARS_PATH} not present in this tree");
        return;
    };
    for spec in FEATURES {
        assert!(
            enterprise.contains(&format!("`{}`", spec.key)),
            "{} has no row in the 25-enterprise.md pinning table",
            spec.key,
        );
        assert!(
            env_vars.contains(&format!("`{}`", spec.env)),
            "{} is undocumented in 22-environment-variables.md",
            spec.env,
        );
    }
}

fn read_optional(rel: &str) -> Option<String> {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).ok()
}
