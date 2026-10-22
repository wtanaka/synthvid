//! Integration tests that check the committed lockfile against the catalogue.

// Cargo links every dependency and dev-dependency of the package into each
// integration test. This one reads the catalogue and the lockfile only.
use synthvid_scene as _;
use synthvid_video as _;

#[cfg(test)]
mod cases;
