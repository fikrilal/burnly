mod adapter;
pub(crate) mod product_variant;

#[allow(
    unused_imports,
    reason = "Antigravity collector is wired before runtime discovery is implemented"
)]
pub(crate) use adapter::AntigravityCollector;
