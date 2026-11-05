//! Shared by the test binaries that inspect rendered expressions.

use topcoat::runtime::expressions;

/// The JavaScript of the expression sites `html` names. The page itself
/// carries only their ids and captured values.
pub fn sources(html: &str) -> String {
    let sources: Vec<_> = expressions()
        .sites()
        .iter()
        .filter(|site| html.contains(&site.id().to_string()))
        .map(|site| site.source())
        .collect();
    sources.join("\n")
}
