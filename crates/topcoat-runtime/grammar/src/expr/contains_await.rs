use syn::{
    ExprBlock, ExprIf,
    visit::{self, Visit},
};

#[derive(Default)]
pub(super) struct ContainsAwait {
    found: bool,
}
impl ContainsAwait {
    pub(super) fn in_if(if_expr: &ExprIf) -> bool {
        let mut visitor = Self::default();
        visitor.visit_expr_if(if_expr);
        visitor.found
    }

    pub(super) fn in_block(block_expr: &ExprBlock) -> bool {
        let mut visitor = Self::default();
        visitor.visit_expr_block(block_expr);
        visitor.found
    }
}

impl<'ats> Visit<'ats> for ContainsAwait {
    fn visit_expr_await(&mut self, node: &'ats syn::ExprAwait) {
        self.found = true;
        visit::visit_expr_await(self, node);
    }

    /// A `raw!` whose JavaScript awaits makes its block await too, so the
    /// function the block lowers to is `async`.
    fn visit_expr_macro(&mut self, node: &'ats syn::ExprMacro) {
        if node.mac.path.is_ident("raw") {
            let js = node.mac.parse_body_with(|input: syn::parse::ParseStream<'_>| {
                let js: syn::LitStr = input.parse()?;
                let _: proc_macro2::TokenStream = input.parse()?;
                Ok(js)
            });
            if js.is_ok_and(|js| Self::awaits(&js.value())) {
                self.found = true;
            }
        }
        visit::visit_expr_macro(self, node);
    }

    fn visit_expr_closure(&mut self, _node: &'ats syn::ExprClosure) {}
}

impl ContainsAwait {
    /// Whether `js` uses the `await` keyword: `await` as a word of its own.
    fn awaits(js: &str) -> bool {
        let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
        js.match_indices("await").any(|(at, _)| {
            !js[..at].ends_with(word) && !js[at + "await".len()..].starts_with(word)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block awaits when a `raw!` in it does, and not for a word containing
    /// `await`.
    #[test]
    fn raw_javascript_that_awaits_makes_its_block_await() {
        let awaits: ExprBlock = syn::parse_quote!({ let x = raw!("await ${y}", 1); });
        assert!(ContainsAwait::in_block(&awaits));
        let named: ExprBlock = syn::parse_quote!({ let x = raw!("awaited(${y})", 1); });
        assert!(!ContainsAwait::in_block(&named));
    }
}
