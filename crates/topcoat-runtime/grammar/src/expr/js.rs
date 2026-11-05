use std::{
    fmt::{self, Write},
    ops::AddAssign,
};

use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, quote};
use topcoat_core_grammar::common::paths::topcoat_runtime;

/// The JavaScript of one expression site, with a hole for each expression
/// captured by the server.
///
/// It lowers to a statically registered site, an arrow function taking the
/// runtime context and one parameter per hole, applied to the captured
/// expressions' own JavaScript. Each use of a capture calls its hole in
/// place, `$0(cx)`, so it runs where and as often as inlined source would.
#[derive(Default)]
pub(super) struct Js {
    source: String,
    /// The captured expressions, in parameter order.
    holes: Vec<Ident>,
}

impl Js {
    pub(super) fn push_str(&mut self, source: &str) {
        self.source.push_str(source);
    }

    pub(super) fn push(&mut self, character: char) {
        self.source.push(character);
    }

    /// Uses the captured expression bound to `ident`.
    pub(super) fn expression(&mut self, ident: &Ident) {
        let index = self
            .holes
            .iter()
            .position(|hole| hole == ident)
            .unwrap_or_else(|| {
                self.holes.push(ident.clone());
                self.holes.len() - 1
            });
        let _ = write!(self.source, "${index}(cx)");
    }

    /// The site's function source.
    fn site_source(&self) -> String {
        let mut site = String::from("(cx");
        for index in 0..self.holes.len() {
            let _ = write!(site, ", ${index}");
        }
        let _ = write!(site, ") => ({})", self.source);
        site
    }
}

impl ToTokens for Js {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let site = self.site_source();
        // A local, since a bare `#topcoat_runtime` inside the repetition
        // would expand to a binding that cannot shadow the imported constant.
        let expr = quote!(#topcoat_runtime::Expr);
        let holes = &self.holes;
        quote! {
            #topcoat_runtime::Js::site(
                #topcoat_runtime::internal::site!(#site),
                ::std::vec![#(::core::clone::Clone::clone(#expr::js(&#holes))),*],
            )
        }
        .to_tokens(tokens);
    }
}

impl fmt::Write for Js {
    fn write_str(&mut self, source: &str) -> fmt::Result {
        self.push_str(source);
        Ok(())
    }
}

impl AddAssign<&str> for Js {
    fn add_assign(&mut self, source: &str) {
        self.push_str(source);
    }
}

#[cfg(test)]
mod tests {
    use proc_macro2::Span;

    use super::*;

    #[test]
    fn each_capture_is_one_hole_called_at_every_use() {
        let first = Ident::new("first", Span::call_site());
        let second = Ident::new("second", Span::call_site());
        let mut js = Js::default();
        js.expression(&first);
        js.push_str(" + ");
        js.expression(&second);
        js.push_str(" * ");
        js.expression(&first);

        assert_eq!(
            js.site_source(),
            "(cx, $0, $1) => ($0(cx) + $1(cx) * $0(cx))",
        );
    }
}
