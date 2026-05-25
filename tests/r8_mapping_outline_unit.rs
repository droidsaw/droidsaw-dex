//! Test consumer for `tests/common/r8_mapping_outline.rs`. The
//! parser lives in the shared `common` module so multiple
//! integration tests (the mapping-paired ratchet harness, the existing
//! oracle ratchet's future revision) can use it without duplicating
//! the code. This file's only job is to drag the parser into a
//! compilation unit where its `#[cfg(test)]` unit tests are
//! discovered.

mod common;

// The parser's unit tests live in `common::r8_mapping_outline::tests`.
// Their results show up under that path when this test binary runs.
