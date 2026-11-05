use std::{collections::BTreeMap, fmt::Write, sync::OnceLock};

use sha2::{Digest, Sha256};

use crate::{Site, js::registered_sites};

/// The URL path prefix
/// [`RouterBuilderRuntimeExt::runtime`](crate::RouterBuilderRuntimeExt::runtime) serves the
/// expressions module under.
const ROUTE_PREFIX: &str = "/_topcoat/runtime";

/// The JavaScript of every runtime expression in the binary, as one script.
///
/// Pages carry runtime expressions as data naming [`Site`]s. This script
/// defines each site's function, so the browser runtime runs expressions
/// without evaluating source and works under a Content-Security-Policy
/// without `'unsafe-eval'`. It must run before the runtime's script.
///
/// The source is deterministic: sites sorted by id, each once. Its file name
/// carries a hash of the source, so it can be cached as immutable.
#[derive(Debug)]
pub struct Expressions {
    sites: Vec<&'static Site>,
    source: String,
    file_name: String,
    path: String,
}

/// Returns the expressions module of every site registered in the binary.
///
/// It is built on first use.
/// [`RouterBuilderRuntimeExt::runtime`](crate::RouterBuilderRuntimeExt::runtime)
/// serves it at [`Expressions::path`] and `topcoat::runtime::script()` loads
/// it. An application serving its own assets can serve [`source`](Expressions::source)
/// under [`file_name`](Expressions::file_name) instead.
///
/// # Panics
///
/// Panics if two registered sites with different sources share an id.
#[must_use]
pub fn expressions() -> &'static Expressions {
    static EXPRESSIONS: OnceLock<Expressions> = OnceLock::new();
    EXPRESSIONS.get_or_init(|| Expressions::new(registered_sites()))
}

impl Expressions {
    /// Builds a module defining the given sites.
    ///
    /// [`expressions()`] builds the module of every registered site. Use this
    /// for a module limited to some sites, such as those of one [`Js`](crate::Js).
    ///
    /// # Panics
    ///
    /// Panics if two sites with different sources share an id.
    #[must_use]
    pub fn new(sites: impl IntoIterator<Item = &'static Site>) -> Self {
        let mut unique = BTreeMap::new();
        for site in sites {
            if let Some(known) = unique.insert(site.id(), site) {
                assert_eq!(
                    known.source(),
                    site.source(),
                    "runtime expression sites share the id {}",
                    site.id(),
                );
            }
        }

        // Several modules may define sites in one document, such as a test
        // harness's, so each adds to the same registry.
        let mut source = String::from(
            "// Topcoat runtime expressions. Generated from the application's sites.\n\
             Object.assign(globalThis.__topcoat_expressions ??= Object.create(null), {\n",
        );
        for (id, site) in &unique {
            let _ = writeln!(source, "\"{id}\": {},", site.source());
        }
        source.push_str("});\n");

        let digest = Sha256::digest(source.as_bytes());
        let mut hash = String::with_capacity(16);
        for byte in &digest[..8] {
            let _ = write!(hash, "{byte:02x}");
        }
        let file_name = format!("expressions-{hash}.js");
        let path = format!("{ROUTE_PREFIX}/{file_name}");
        Self {
            sites: unique.into_values().collect(),
            source,
            file_name,
            path,
        }
    }

    /// The sites the module defines, sorted by id.
    #[must_use]
    pub fn sites(&self) -> &[&'static Site] {
        &self.sites
    }

    /// The module's JavaScript.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The file name, `expressions-<hash>.js`, where the hash covers the
    /// source.
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// The URL path `.runtime()` serves the module at,
    /// `/_topcoat/runtime/<file_name>`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

#[cfg(feature = "router")]
pub(crate) use route::ExpressionsRoute;

#[cfg(feature = "router")]
mod route {
    use topcoat_core::context::Cx;
    use topcoat_router::{
        Body, HeaderValue, Method, Methods, Path, PathBuf, Route, RouteFuture, RouteId,
        header::{CACHE_CONTROL, CONTENT_TYPE},
        response::Response,
    };

    use super::expressions;

    /// Serves [`expressions()`] at its hashed path with immutable caching.
    #[derive(Debug)]
    pub(crate) struct ExpressionsRoute {
        id: RouteId,
        path: PathBuf,
    }

    impl ExpressionsRoute {
        pub(crate) fn new() -> Self {
            Self {
                id: RouteId::new(),
                path: Path::new(expressions().path()).to_owned(),
            }
        }
    }

    impl Route for ExpressionsRoute {
        fn id(&self) -> RouteId {
            self.id
        }

        fn methods(&self) -> Methods<'_> {
            Methods::Only(&[Method::GET])
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn handle<'cx>(&'cx self, _cx: &'cx Cx, _body: Body) -> RouteFuture<'cx> {
            Box::pin(async move {
                let mut response = Response::new(Body::from(expressions().source()));
                let headers = response.headers_mut();
                headers.insert(
                    CONTENT_TYPE,
                    HeaderValue::from_static("text/javascript; charset=utf-8"),
                );
                headers.insert(
                    CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
                Ok(response)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_is_sorted_deduplicated_and_hashed() {
        let first = crate::__site!("(cx) => (1)");
        let second = crate::__site!("(cx) => (2)");
        let module = Expressions::new([second, first, second]);
        let reversed = Expressions::new([first, second]);

        assert_eq!(module.source(), reversed.source());
        assert_eq!(module.file_name(), reversed.file_name());
        assert_eq!(module.source().matches("(cx) => (2)").count(), 1);
        let (low, high) = if first.id() < second.id() {
            (first, second)
        } else {
            (second, first)
        };
        let position = |site: &Site| module.source().find(&site.id().to_string()).unwrap();
        assert!(position(low) < position(high));
        let hash = module
            .file_name()
            .strip_prefix("expressions-")
            .and_then(|name| name.strip_suffix(".js"))
            .unwrap();
        assert_eq!(hash.len(), 16);
        assert_eq!(
            module.path(),
            format!("/_topcoat/runtime/{}", module.file_name())
        );

        assert!(expressions().source().contains(&first.id().to_string()));
    }
}
