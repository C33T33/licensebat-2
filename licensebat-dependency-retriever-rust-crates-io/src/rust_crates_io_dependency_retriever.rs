use futures::{
    future::{self, BoxFuture},
    FutureExt, TryFutureExt,
};
use licensebat_core::{Comment, Dependency, DependencyRetriever, RetrievedDependency};
use reqwest::Client;
use serde_json::Value;
use tracing::instrument;

pub struct RustCratesIoDependencyRetriever {
    client: Client,
}

impl Default for RustCratesIoDependencyRetriever {
    /// Creates a new [`RustCratesIoDependencyRetriever`].
    fn default() -> Self {
        Self::new()
    }
}

impl RustCratesIoDependencyRetriever {
    /// Creates a new [`RustCratesIoDependencyRetriever`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_client(Client::new())
    }

    /// Creates a new [`DependencyRetriever`] using the given [`reqwest::Client`].
    #[must_use]
    pub const fn with_client(client: Client) -> Self {
        Self { client }
    }
}

impl DependencyRetriever for RustCratesIoDependencyRetriever {
    type Error = std::convert::Infallible;
    type Future = BoxFuture<'static, Result<RetrievedDependency, Self::Error>>;

    #[instrument(skip(self), level = "debug")]
    fn get_dependency(&self, dep_name: &str, dep_version: &str) -> Self::Future {
        let url = format!(
            "https://crates.io/api/v1/crates/{}/{}",
            dep_name, dep_version
        );

        let dependency = Dependency {
            name: dep_name.to_string(),
            version: dep_version.to_string(),
        };

        let dep_clone = dependency.clone();

        self.client
            .get(&url)
            .header("User-Agent", "licensebat-cli (licensebat.com)")
            .send()
            .and_then(reqwest::Response::json)
            .map_ok(|metadata: Value| {
                let license = metadata["version"]["license"].clone();
                vec![license.as_str().unwrap().to_string()]
                // TODO: GET LICENSE IN CASE OF non-standard license
                // we should get the repo info, get the cargo.toml, read the license_file key, get the file,
                // read it and use askalono to get the license.
                // TODO: ADD SUPPORT FOR MULTIPLE LICENSES
            })
            .map_ok(move |licenses| build_retrieved_dependency(&dep_clone, Some(licenses), None))
            .or_else(move |e| future::ok(build_retrieved_dependency(&dependency, None, Some(e))))
            .boxed()
    }
}

#[instrument(level = "debug")]
fn build_retrieved_dependency(
    dependency: &Dependency,
    licenses: Option<Vec<String>>,
    error: Option<reqwest::Error>,
) -> RetrievedDependency {
    let url = format!(
        "https://crates.io/crates/{}/{}",
        dependency.name, dependency.version
    );

    let has_licenses = licenses.is_some();

    // TODO: THIS SHOULD BE EXTRACTED AS IT SEEMS TO BE THE SAME FOR ALL DEPENDENCY TYPES
    RetrievedDependency {
        name: dependency.name.clone(),
        version: dependency.version.clone(),
        url: Some(url),
        dependency_type: "npm".to_owned(),
        validated: false,
        is_valid: has_licenses && error.is_none(),
        is_ignored: false,
        error: if let Some(err) = error {
            Some(err.to_string())
        } else if has_licenses {
            None
        } else {
            Some("No License".to_owned())
        },
        licenses: if has_licenses {
            licenses
        } else {
            Some(vec!["NO-LICENSE".to_string()])
        },
        comment: if has_licenses {
            None
        } else {
            Some(Comment::removable("Consider **ignoring** this specific dependency. You can also accept the **NO-LICENSE** key to avoid these issues."))
        },
    }
}
