#![doc = include_str!("../docs/runtime.md")]

pub use topcoat_runtime::*;
pub use topcoat_runtime_macro::*;

/// The `<script>` tags loading the browser runtime.
///
/// The [expressions module](expressions()) comes first, as a classic deferred
/// script, so the functions it defines are registered before the runtime
/// module starts. Neither script evaluates source at runtime, so a
/// Content-Security-Policy of `script-src 'self'` covers both when the
/// application serves its assets.
///
/// # Panics
///
/// Panics if the router was built without
/// [`runtime()`](RouterBuilderRuntimeExt::runtime).
#[cfg(feature = "view")]
#[topcoat::view::component]
pub async fn script(cx: &topcoat::context::Cx) -> topcoat::Result<impl topcoat::view::View> {
    assert!(
        topcoat::context::try_context::<RuntimeSetup>(cx).is_some(),
        "the browser runtime is not set up on this router; call `.runtime()` on the router builder",
    );
    Ok(topcoat::view::view! {
        <script defer=(true) src=(expressions().path())></script>
        <script
            type="module"
            src=(topcoat::runtime::SCRIPT)
            data-topcoat-usize-bits=(usize::BITS)
        ></script>
    })
}
