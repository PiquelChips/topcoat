use topcoat_core::context::Cx;
use topcoat_view::{Attribute, AttributeKeyViewParts, AttributeViewParts, PartsWriter, Unescaped};

use crate::{Event, Expr, Js};

pub trait EventHandlerFn {}

impl<T, R> EventHandlerFn for T where T: FnOnce(Event) -> R {}

/// An attribute that attaches a browser event handler to an element.
pub struct EventHandler<K> {
    key: K,
    js: Js,
}

impl<K> EventHandler<K> {
    /// A handler compiled from an expression, as in `@click=$(|e| ...)`.
    #[inline]
    pub fn new<F>(key: K, value: Expr<F>) -> Self
    where
        F: EventHandlerFn,
    {
        Self { key, js: value.js }
    }

    /// A handler from JavaScript evaluating to the handler function, as in
    /// `@click="(e) => ..."`, which `view!` registers with [`js!`](crate::js!).
    #[inline]
    pub fn from_js(key: K, js: Js) -> Self {
        Self { key, js }
    }
}

impl<K> AttributeViewParts for EventHandler<K>
where
    K: AttributeKeyViewParts,
{
    #[inline]
    fn into_view_parts(self, cx: &Cx, parts: &mut PartsWriter<'_>) {
        Attribute::new(
            (Unescaped::new_unchecked("data-topcoat-on:"), self.key),
            self.js,
        )
        .into_view_parts(cx, parts);
    }
}
