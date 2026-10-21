//! Integration tests for the verify command.

// Cargo links every dependency and dev-dependency of the package into each
// integration test. This one drives the built binary and calls none of them.
use synthvid_catalog as _;
use synthvid_scene as _;
use synthvid_video as _;

#[cfg(test)]
mod cases;
