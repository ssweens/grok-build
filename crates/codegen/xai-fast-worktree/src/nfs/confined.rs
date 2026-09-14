//! Fd-relative helpers + the single owned deleter used by daemon-down `rm`
//! and `clean-artifacts`. Never a weaker sibling of `grove_git::delete_owned`.
/// Allowlist validation: ids flow into paths, pin refs
/// (`refs/grok/worktrees/{id}`), and marker files, so anything outside
/// `[A-Za-z0-9._-]` — whitespace, control bytes, ref metachars — is rejected.
pub fn is_safe_worktree_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && !id.contains("..")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}
