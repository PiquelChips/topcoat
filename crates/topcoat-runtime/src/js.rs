use std::fmt;

use serde::Serialize;
use topcoat_core::context::Cx;
use topcoat_view::{AttributeValueViewParts, PartsWriter};

/// The browser side of a runtime expression, as data.
///
/// A `Js` is a tree whose nodes are either a [`Site`] applied to its holes or
/// a captured value. It renders as JSON naming sites by their [`SiteId`]; the
/// browser runtime finds each site's function in the
/// [expressions module](crate::expressions()) and calls it with its holes. No
/// JavaScript source is ever rendered into the page, so the runtime works
/// under a Content-Security-Policy without `'unsafe-eval'`.
///
/// Build one with [`expr!`](https://docs.rs/topcoat/latest/topcoat/runtime/macro.expr.html),
/// [`js!`](crate::js!), [`Js::site`], or [`Js::value`].
#[derive(Debug, Clone)]
pub struct Js(Node);

#[derive(Debug, Clone)]
enum Node {
    /// A site's function, called with one argument per hole.
    Site { site: &'static Site, holes: Vec<Js> },
    /// A captured value as JSON, hydrated on the client.
    Value(String),
}

impl Js {
    /// Applies a registered site to its holes, in parameter order.
    ///
    /// Each hole is passed to the site's function as a thunk, so its value is
    /// computed where and whenever the site's source calls it. See [`Site`].
    #[must_use]
    pub fn site(site: &'static Site, holes: Vec<Js>) -> Self {
        Self(Node::Site { site, holes })
    }

    /// A captured value, serialized now so the Rust expression can consume
    /// it afterwards.
    ///
    /// # Panics
    ///
    /// Panics if the value fails to serialize.
    #[must_use]
    pub fn value(value: &(impl Serialize + ?Sized)) -> Self {
        let json = serde_json::to_string(value).expect("failed to serialize surrogate value");
        Self(Node::Value(json))
    }

    /// Returns the JSON the browser runtime resolves this expression from.
    ///
    /// A site is `["<id>", <hole>...]` and a captured value is
    /// `{"v": <value>}`.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut json = String::new();
        self.write_json(&mut json);
        json
    }

    fn write_json(&self, json: &mut String) {
        match &self.0 {
            Node::Site { site, holes } => {
                json.push_str("[\"");
                json.push_str(&site.id.to_string());
                json.push('"');
                for hole in holes {
                    json.push(',');
                    hole.write_json(json);
                }
                json.push(']');
            }
            Node::Value(value) => {
                json.push_str("{\"v\":");
                json.push_str(value);
                json.push('}');
            }
        }
    }

    /// Returns the sites this expression uses, including those of its holes,
    /// each once, in order of first use.
    #[must_use]
    pub fn sites(&self) -> Vec<&'static Site> {
        let mut sites = Vec::new();
        self.collect_sites(&mut sites);
        sites
    }

    fn collect_sites(&self, sites: &mut Vec<&'static Site>) {
        if let Node::Site { site, holes } = &self.0 {
            if !sites.iter().any(|known| known.id == site.id) {
                sites.push(site);
            }
            for hole in holes {
                hole.collect_sites(sites);
            }
        }
    }

    /// Writes the JSON through `parts`, sealed for the writer's current
    /// context: a marker comment's body or a double-quoted attribute value.
    pub(crate) fn write(&self, parts: &mut PartsWriter<'_>) {
        parts.push_string(self.to_json());
    }
}

/// The JSON as the value of a `data-topcoat-*` attribute, like an event
/// handler or a bind expression.
impl AttributeValueViewParts for Js {
    #[inline]
    fn attribute_present(&self) -> bool {
        true
    }

    #[inline]
    fn into_view_parts(self, _cx: &Cx, parts: &mut PartsWriter<'_>) {
        self.write(parts);
    }
}

/// JavaScript known at compile time: one function of a runtime expression.
///
/// The source is an arrow function taking the runtime context `cx` and one
/// parameter per hole, `$0`, `$1`, and so on. Each hole is itself an
/// expression of the runtime context, so the body calls `$0(cx)` at every
/// place the hole's value is needed. Holes keep the evaluation order,
/// short-circuiting, and signal tracking of inlined source.
///
/// ```js
/// (cx, $0, $1) => ($0(cx).get().add($1(cx)))
/// ```
///
/// Sites are declared with [`js!`](crate::js!) or by
/// [`expr!`](https://docs.rs/topcoat/latest/topcoat/runtime/macro.expr.html),
/// which register them statically, and are served together as the
/// [expressions module](crate::expressions()).
#[derive(Debug)]
pub struct Site {
    id: SiteId,
    source: &'static str,
}

impl Site {
    /// Declares a site whose id is a hash of its source.
    ///
    /// Use [`js!`](crate::js!) instead, which also registers the site. An
    /// unregistered site is missing from the expressions module, so the
    /// browser runtime fails when it resolves it.
    #[doc(hidden)]
    #[must_use]
    pub const fn new(source: &'static str) -> Self {
        Self {
            id: SiteId::of(source),
            source,
        }
    }

    /// The id the browser runtime finds this site's function by.
    #[must_use]
    pub const fn id(&self) -> SiteId {
        self.id
    }

    /// The function's JavaScript source.
    #[must_use]
    pub const fn source(&self) -> &'static str {
        self.source
    }
}

/// Every site registered with [`js!`](crate::js!) or `expr!` in the binary.
pub(crate) fn registered_sites() -> impl Iterator<Item = &'static Site> {
    inventory::iter::<&'static Site>().copied()
}

inventory::collect!(&'static Site);

/// Identifies a [`Site`] by a 64-bit FNV-1a hash of its source.
///
/// Displays as 16 lowercase hexadecimal digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SiteId(u64);

impl SiteId {
    const fn of(source: &str) -> Self {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;

        let bytes = source.as_bytes();
        let mut hash = OFFSET;
        let mut index = 0;
        while index < bytes.len() {
            hash ^= bytes[index] as u64;
            hash = hash.wrapping_mul(PRIME);
            index += 1;
        }
        Self(hash)
    }
}

impl fmt::Display for SiteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Declares and registers a [`Site`], returning `&'static Site`.
///
/// The source must be a string literal holding an arrow function, as
/// described on [`Site`].
#[doc(hidden)]
#[macro_export]
macro_rules! __site {
    ($source:expr) => {{
        static SITE: $crate::Site = $crate::Site::new($source);
        $crate::internal::inventory::submit! { &SITE }
        &SITE
    }};
}

/// Creates a [`Js`] from a JavaScript expression known at compile time.
///
/// The expression can use the runtime context as `cx`. It is registered as a
/// [`Site`] without holes, so it is part of the
/// [expressions module](crate::expressions()) and nothing but its id renders
/// into the page.
///
/// ```rust
/// use topcoat::runtime::{Expr, js};
///
/// let ready = Expr::evaluate(|| true, js!("cx.hydrate(true)"));
/// ```
#[macro_export]
macro_rules! js {
    ($source:literal) => {
        $crate::Js::site(
            $crate::__site!(::core::concat!("(cx) => (", $source, ")")),
            ::std::vec::Vec::new(),
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_sites_and_values_as_data() {
        let inner = js!("cx.hydrate(1)");
        let site = crate::__site!("(cx, $0, $1) => ($0(cx).add($1(cx)))");
        let js = Js::site(site, vec![inner.clone(), Js::value(&"\"<>&\n")]);

        assert_eq!(
            js.to_json(),
            format!(
                r#"["{}",["{}"],{{"v":"\"<>&\n"}}]"#,
                site.id(),
                inner.sites()[0].id(),
            ),
        );
        assert_eq!(js.sites().len(), 2);
        assert!(registered_sites().any(|known| known.id() == site.id()));
    }

    #[test]
    fn ids_are_stable_hashes_of_the_source() {
        assert_eq!(SiteId::of("").to_string(), "cbf29ce484222325");
        assert_eq!(SiteId::of("a").to_string(), "af63dc4c8601ec8c");
    }
}
