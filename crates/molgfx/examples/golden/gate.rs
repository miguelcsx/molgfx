//! The pure verdict shared by diagnostic and release-gating corpus runs.

use std::io;

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;

pub(crate) fn verify_comparison(
    binding: bool,
    failures: usize,
    require_reference: bool,
) -> io::Result<()> {
    if !binding && require_reference {
        return Err(io::Error::other(format!(
            "GOLDEN: UNCERTIFIED — reference identity cannot be verified; {failures} scene(s) drifted"
        )));
    }
    if binding && failures > 0 {
        return Err(io::Error::other(format!(
            "{failures} golden scene(s) drifted"
        )));
    }
    Ok(())
}
