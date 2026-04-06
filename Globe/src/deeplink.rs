//! Deep linking — shareable addresses for Omnidea content.
//!
//! Every piece of content on Omnidea gets a shareable address. The `omnidea://`
//! URI scheme lets users share links that open directly in the right app.
//!
//! # URI Scheme
//!
//! ```text
//! omnidea://{app}/{resource_type}/{resource_id}?key=value
//! omnidea://plexus/post/abc123
//! omnidea://luminaria/design/def456
//! omnidea://invite?relay=ws://host&token=xyz
//! ```
//!
//! # Globe Names
//!
//! Human-readable addresses using the `.idea` TLD:
//!
//! ```text
//! sam.idea              → resolves to a pubkey
//! sam.idea/portfolio    → name + path
//! community.idea/docs     → community name + path
//! ```
//!
//! # URI Routing
//!
//! Apps register `UriHandler` implementations. The `UriRouter` matches incoming
//! URIs to handlers and returns a `UriAction` describing what to do.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::GlobeError;
use crate::gospel::GospelRegistry;
use crate::name::parse_name_record;

// ---------------------------------------------------------------------------
// OmnideaUri
// ---------------------------------------------------------------------------

/// A parsed `omnidea://` URI.
///
/// Generic form: `omnidea://{app}/{resource_type}/{resource_id}?key=value`
///
/// Special forms:
/// - `omnidea://invite?...` (invitation links — no resource_type/id)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmnideaUri {
    /// The target app (e.g., "plexus", "luminaria", "omny", "invite").
    pub app: String,
    /// The resource type within the app (e.g., "post", "design", "community").
    pub resource_type: Option<String>,
    /// The resource identifier.
    pub resource_id: Option<String>,
    /// Query parameters.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
}

const SCHEME: &str = "omnidea://";

impl OmnideaUri {
    /// Parse an `omnidea://` URI string.
    pub fn parse(uri: &str) -> Result<Self, GlobeError> {
        let body = uri
            .strip_prefix(SCHEME)
            .ok_or_else(|| GlobeError::InvalidMessage(format!("not an omnidea URI: {uri}")))?;

        if body.is_empty() {
            return Err(GlobeError::InvalidMessage(
                "empty omnidea URI".into(),
            ));
        }

        // Split path from query string.
        let (path, query) = match body.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (body, None),
        };

        // Parse query params.
        let params = match query {
            Some(q) => parse_query(q),
            None => HashMap::new(),
        };

        // Split path segments (filter out empty segments from trailing slashes).
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match segments.len() {
            0 => Err(GlobeError::InvalidMessage(
                "omnidea URI has no app segment".into(),
            )),
            1 => Ok(Self {
                app: segments[0].to_string(),
                resource_type: None,
                resource_id: None,
                params,
            }),
            2 => Ok(Self {
                app: segments[0].to_string(),
                resource_type: Some(segments[1].to_string()),
                resource_id: None,
                params,
            }),
            _ => Ok(Self {
                app: segments[0].to_string(),
                resource_type: Some(segments[1].to_string()),
                resource_id: Some(segments[2..].join("/")),
                params,
            }),
        }
    }

    /// Construct from parts.
    pub fn new(app: &str, resource_type: Option<&str>, resource_id: Option<&str>) -> Self {
        Self {
            app: app.to_string(),
            resource_type: resource_type.map(|s| s.to_string()),
            resource_id: resource_id.map(|s| s.to_string()),
            params: HashMap::new(),
        }
    }

    /// Add a query parameter.
    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }

    /// Serialize back to a URI string.
    pub fn to_uri(&self) -> String {
        let mut uri = format!("{SCHEME}{}", self.app);

        if let Some(rt) = &self.resource_type {
            uri.push('/');
            uri.push_str(rt);
        }
        if let Some(ri) = &self.resource_id {
            uri.push('/');
            uri.push_str(ri);
        }

        if !self.params.is_empty() {
            uri.push('?');
            let mut pairs: Vec<_> = self.params.iter().collect();
            pairs.sort_by_key(|(k, _)| (*k).clone());
            let qs: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            uri.push_str(&qs.join("&"));
        }

        uri
    }

    /// Whether this is an invitation URI.
    pub fn is_invite(&self) -> bool {
        self.app == "invite"
    }
}

impl std::fmt::Display for OmnideaUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_uri())
    }
}

/// Parse a query string into key-value pairs.
fn parse_query(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if !key.is_empty() {
                map.insert(key.to_string(), value.to_string());
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------
// GlobeName
// ---------------------------------------------------------------------------

/// A parsed `.idea` domain name with optional path.
///
/// ```text
/// sam.idea              → name="sam", path=None
/// sam.idea/portfolio    → name="sam", path=Some("portfolio")
/// community.idea/docs     → name="community", path=Some("docs")
/// shop.sam.idea         → name="shop.sam", path=None
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobeName {
    /// The name portion (everything before `.idea` and before any `/`).
    pub name: String,
    /// Optional path after the `.idea` domain.
    pub path: Option<String>,
}

const IDEA_TLD: &str = ".idea";

impl GlobeName {
    /// Parse a `.idea` address string.
    ///
    /// Accepts `sam.idea`, `sam.idea/portfolio`, `shop.sam.idea/path/to/thing`.
    pub fn parse(input: &str) -> Result<Self, GlobeError> {
        // Split path from the domain.
        let (domain, path) = match input.split_once('/') {
            Some((d, p)) => {
                let path = if p.is_empty() { None } else { Some(p.to_string()) };
                (d, path)
            }
            None => (input, None),
        };

        // Must end with .idea
        if !domain.ends_with(IDEA_TLD) {
            return Err(GlobeError::InvalidMessage(format!(
                "not a .idea address: {input}"
            )));
        }

        let name = &domain[..domain.len() - IDEA_TLD.len()];
        if name.is_empty() {
            return Err(GlobeError::InvalidMessage(
                ".idea address has no name".into(),
            ));
        }

        // Validate: name must contain only alphanumeric, hyphens, dots (for subdomains).
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        {
            return Err(GlobeError::InvalidMessage(format!(
                "invalid characters in .idea name: {name}"
            )));
        }

        // Must not start or end with dot/hyphen.
        if name.starts_with('.') || name.starts_with('-')
            || name.ends_with('.') || name.ends_with('-')
        {
            return Err(GlobeError::InvalidMessage(format!(
                "invalid .idea name: {name}"
            )));
        }

        Ok(Self {
            name: name.to_string(),
            path,
        })
    }

    /// The full `.idea` domain (without path).
    pub fn domain(&self) -> String {
        format!("{}{IDEA_TLD}", self.name)
    }

    /// The Globe name registry key.
    ///
    /// Maps to the existing Globe naming system: `name.idea` (TLD is "idea").
    pub fn registry_key(&self) -> String {
        self.domain()
    }

    /// Resolve this Globe name to a public key via the gospel registry.
    ///
    /// Returns `None` if the name is not found in the local cache.
    pub fn resolve(&self, registry: &GospelRegistry) -> Option<String> {
        let event = registry.lookup_name(&self.registry_key())?;
        let record = parse_name_record(&event).ok()?;
        record.target.or(Some(record.owner))
    }

    /// Full address string (domain + optional path).
    pub fn to_address(&self) -> String {
        match &self.path {
            Some(p) => format!("{}/{p}", self.domain()),
            None => self.domain(),
        }
    }
}

impl std::fmt::Display for GlobeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_address())
    }
}

// ---------------------------------------------------------------------------
// UriAction
// ---------------------------------------------------------------------------

/// What to do when a URI is handled.
///
/// Returned by [`UriHandler::handle`] after matching a URI. The app layer
/// uses this to decide which view to show or which action to take.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UriAction {
    /// Navigate to a specific view in an app.
    Navigate {
        /// Target app identifier.
        app: String,
        /// View name within the app.
        view: String,
        /// Additional parameters for the view.
        params: HashMap<String, String>,
    },
    /// Open a specific piece of content by ID.
    Open {
        /// The content identifier to open.
        content_id: String,
    },
    /// Process an invitation.
    Invite {
        /// The one-time invitation code.
        code: String,
    },
    /// URI was not recognized by any handler.
    Unknown,
}

// ---------------------------------------------------------------------------
// UriHandler + UriRouter
// ---------------------------------------------------------------------------

/// Trait for URI handlers. Apps implement this to handle their own URIs.
pub trait UriHandler: Send + Sync {
    /// Whether this handler can handle the given URI.
    fn can_handle(&self, uri: &OmnideaUri) -> bool;
    /// Handle the URI and return an action.
    fn handle(&self, uri: &OmnideaUri) -> UriAction;
}

/// Routes URIs to registered handlers.
///
/// Apps register handlers at runtime. When a URI arrives, the router finds
/// the first handler that can handle it and returns the resulting action.
pub struct UriRouter {
    handlers: Vec<Box<dyn UriHandler>>,
}

impl UriRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Register a handler.
    pub fn register(&mut self, handler: Box<dyn UriHandler>) {
        self.handlers.push(handler);
    }

    /// Route a URI to the appropriate handler.
    ///
    /// Returns `UriAction::Unknown` if no handler matches.
    pub fn route(&self, uri: &OmnideaUri) -> UriAction {
        for handler in &self.handlers {
            if handler.can_handle(uri) {
                return handler.handle(uri);
            }
        }
        UriAction::Unknown
    }

    /// Route a URI string (parses first).
    pub fn route_str(&self, uri: &str) -> Result<UriAction, GlobeError> {
        let parsed = OmnideaUri::parse(uri)?;
        Ok(self.route(&parsed))
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for UriRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LinkBuilder
// ---------------------------------------------------------------------------

/// Convenience methods for building shareable Omnidea links.
pub struct LinkBuilder;

impl LinkBuilder {
    /// Build a link to a Plexus post.
    pub fn post(event_id: &str) -> OmnideaUri {
        OmnideaUri::new("plexus", Some("post"), Some(event_id))
    }

    /// Build a link to a Luminaria design.
    pub fn design(event_id: &str) -> OmnideaUri {
        OmnideaUri::new("luminaria", Some("design"), Some(event_id))
    }

    /// Build a link to a community page.
    pub fn community(community_id: &str) -> OmnideaUri {
        OmnideaUri::new("omny", Some("community"), Some(community_id))
    }

    /// Build a link to a user profile via Globe name.
    pub fn profile(name: &str) -> GlobeName {
        GlobeName {
            name: name.to_string(),
            path: None,
        }
    }

    /// Build an invitation link.
    pub fn invite(relay_url: &str, token: &str, inviter: &str) -> OmnideaUri {
        OmnideaUri::new("invite", None, None)
            .with_param("relay", relay_url)
            .with_param("token", token)
            .with_param("inviter", inviter)
    }

    /// Build a generic app link.
    pub fn app(app: &str, resource_type: &str, resource_id: &str) -> OmnideaUri {
        OmnideaUri::new(app, Some(resource_type), Some(resource_id))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::OmniEvent;
    use crate::gospel::GospelConfig;
    use crate::kind;

    // -- OmnideaUri tests --

    #[test]
    fn parse_full_uri() {
        let uri = OmnideaUri::parse("omnidea://plexus/post/abc123").unwrap();
        assert_eq!(uri.app, "plexus");
        assert_eq!(uri.resource_type, Some("post".into()));
        assert_eq!(uri.resource_id, Some("abc123".into()));
        assert!(uri.params.is_empty());
    }

    #[test]
    fn parse_uri_no_resource_id() {
        let uri = OmnideaUri::parse("omnidea://plexus/post").unwrap();
        assert_eq!(uri.app, "plexus");
        assert_eq!(uri.resource_type, Some("post".into()));
        assert_eq!(uri.resource_id, None);
    }

    #[test]
    fn parse_uri_app_only() {
        let uri = OmnideaUri::parse("omnidea://invite").unwrap();
        assert_eq!(uri.app, "invite");
        assert_eq!(uri.resource_type, None);
        assert_eq!(uri.resource_id, None);
    }

    #[test]
    fn parse_uri_with_params() {
        let uri = OmnideaUri::parse(
            "omnidea://invite?relay=ws://localhost:8080&token=abc123&inviter=cpub_test",
        )
        .unwrap();
        assert_eq!(uri.app, "invite");
        assert!(uri.is_invite());
        assert_eq!(uri.params.get("relay"), Some(&"ws://localhost:8080".into()));
        assert_eq!(uri.params.get("token"), Some(&"abc123".into()));
        assert_eq!(uri.params.get("inviter"), Some(&"cpub_test".into()));
    }

    #[test]
    fn parse_uri_with_path_and_params() {
        let uri =
            OmnideaUri::parse("omnidea://plexus/post/abc123?highlight=true").unwrap();
        assert_eq!(uri.app, "plexus");
        assert_eq!(uri.resource_type, Some("post".into()));
        assert_eq!(uri.resource_id, Some("abc123".into()));
        assert_eq!(uri.params.get("highlight"), Some(&"true".into()));
    }

    #[test]
    fn parse_invalid_scheme() {
        assert!(OmnideaUri::parse("https://example.com").is_err());
        assert!(OmnideaUri::parse("nostr://relay").is_err());
    }

    #[test]
    fn parse_empty_body() {
        assert!(OmnideaUri::parse("omnidea://").is_err());
    }

    #[test]
    fn uri_round_trip() {
        let uri = OmnideaUri::new("plexus", Some("post"), Some("abc123"));
        let s = uri.to_uri();
        let parsed = OmnideaUri::parse(&s).unwrap();
        assert_eq!(uri, parsed);
    }

    #[test]
    fn uri_with_params_round_trip() {
        let uri = OmnideaUri::new("invite", None, None)
            .with_param("token", "abc")
            .with_param("relay", "ws://host");
        let s = uri.to_uri();
        let parsed = OmnideaUri::parse(&s).unwrap();
        assert_eq!(uri.params, parsed.params);
    }

    #[test]
    fn uri_display() {
        let uri = OmnideaUri::new("plexus", Some("post"), Some("abc123"));
        assert_eq!(uri.to_string(), "omnidea://plexus/post/abc123");
    }

    #[test]
    fn uri_serde_round_trip() {
        let uri = OmnideaUri::new("luminaria", Some("design"), Some("xyz"));
        let json = serde_json::to_string(&uri).unwrap();
        let loaded: OmnideaUri = serde_json::from_str(&json).unwrap();
        assert_eq!(uri, loaded);
    }

    #[test]
    fn uri_trailing_slash() {
        let uri = OmnideaUri::parse("omnidea://plexus/post/abc123/").unwrap();
        assert_eq!(uri.resource_id, Some("abc123".into()));
    }

    #[test]
    fn uri_nested_resource_id() {
        let uri = OmnideaUri::parse("omnidea://plexus/post/abc/def/ghi").unwrap();
        assert_eq!(uri.resource_id, Some("abc/def/ghi".into()));
    }

    // -- GlobeName tests --

    #[test]
    fn parse_globe_name_simple() {
        let name = GlobeName::parse("sam.idea").unwrap();
        assert_eq!(name.name, "sam");
        assert_eq!(name.path, None);
        assert_eq!(name.domain(), "sam.idea");
    }

    #[test]
    fn parse_globe_name_with_path() {
        let name = GlobeName::parse("sam.idea/portfolio").unwrap();
        assert_eq!(name.name, "sam");
        assert_eq!(name.path, Some("portfolio".into()));
        assert_eq!(name.to_address(), "sam.idea/portfolio");
    }

    #[test]
    fn parse_globe_name_subdomain() {
        let name = GlobeName::parse("shop.sam.idea").unwrap();
        assert_eq!(name.name, "shop.sam");
        assert_eq!(name.path, None);
    }

    #[test]
    fn parse_globe_name_subdomain_with_path() {
        let name = GlobeName::parse("community.idea/docs/intro").unwrap();
        assert_eq!(name.name, "community");
        assert_eq!(name.path, Some("docs/intro".into()));
    }

    #[test]
    fn parse_globe_name_not_idea_tld() {
        assert!(GlobeName::parse("sam.com").is_err());
        assert!(GlobeName::parse("sam.net").is_err());
    }

    #[test]
    fn parse_globe_name_empty_name() {
        assert!(GlobeName::parse(".idea").is_err());
    }

    #[test]
    fn parse_globe_name_invalid_chars() {
        assert!(GlobeName::parse("rob by.idea").is_err());
        assert!(GlobeName::parse("rob@by.idea").is_err());
    }

    #[test]
    fn parse_globe_name_invalid_edges() {
        assert!(GlobeName::parse("-sam.idea").is_err());
        assert!(GlobeName::parse("sam-.idea").is_err());
        assert!(GlobeName::parse(".sam.idea").is_err());
    }

    #[test]
    fn globe_name_display() {
        let name = GlobeName::parse("sam.idea/portfolio").unwrap();
        assert_eq!(name.to_string(), "sam.idea/portfolio");
    }

    #[test]
    fn globe_name_serde_round_trip() {
        let name = GlobeName::parse("sam.idea/portfolio").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        let loaded: GlobeName = serde_json::from_str(&json).unwrap();
        assert_eq!(name, loaded);
    }

    #[test]
    fn globe_name_trailing_slash() {
        let name = GlobeName::parse("sam.idea/").unwrap();
        assert_eq!(name.name, "sam");
        assert_eq!(name.path, None);
    }

    #[test]
    fn globe_name_resolve_via_registry() {
        let config = GospelConfig::default();
        let registry = GospelRegistry::new(&config);

        let pubkey = "a".repeat(64);

        // Insert a name claim event.
        let event = OmniEvent {
            id: "name-event-1".into(),
            author: pubkey.clone(),
            created_at: 1000,
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.idea".into()],
                vec!["target".into(), pubkey.clone()],
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        registry.insert(&event);

        let name = GlobeName::parse("sam.idea").unwrap();
        let resolved = name.resolve(&registry);
        assert_eq!(resolved, Some(pubkey));
    }

    #[test]
    fn globe_name_resolve_not_found() {
        let config = GospelConfig::default();
        let registry = GospelRegistry::new(&config);

        let name = GlobeName::parse("nobody.idea").unwrap();
        assert_eq!(name.resolve(&registry), None);
    }

    // -- UriAction tests --

    #[test]
    fn uri_action_serde() {
        let action = UriAction::Navigate {
            app: "plexus".into(),
            view: "post".into(),
            params: HashMap::new(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let loaded: UriAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, loaded);
    }

    // -- UriRouter tests --

    struct PlexusHandler;
    impl UriHandler for PlexusHandler {
        fn can_handle(&self, uri: &OmnideaUri) -> bool {
            uri.app == "plexus"
        }
        fn handle(&self, uri: &OmnideaUri) -> UriAction {
            match (&uri.resource_type, &uri.resource_id) {
                (Some(rt), Some(ri)) => UriAction::Navigate {
                    app: "plexus".into(),
                    view: rt.clone(),
                    params: {
                        let mut p = uri.params.clone();
                        p.insert("id".into(), ri.clone());
                        p
                    },
                },
                _ => UriAction::Unknown,
            }
        }
    }

    struct InviteHandler;
    impl UriHandler for InviteHandler {
        fn can_handle(&self, uri: &OmnideaUri) -> bool {
            uri.is_invite()
        }
        fn handle(&self, uri: &OmnideaUri) -> UriAction {
            match uri.params.get("token") {
                Some(code) => UriAction::Invite {
                    code: code.clone(),
                },
                None => UriAction::Unknown,
            }
        }
    }

    #[test]
    fn router_routes_to_handler() {
        let mut router = UriRouter::new();
        router.register(Box::new(PlexusHandler));

        let uri = OmnideaUri::parse("omnidea://plexus/post/abc123").unwrap();
        let action = router.route(&uri);

        match action {
            UriAction::Navigate { app, view, params } => {
                assert_eq!(app, "plexus");
                assert_eq!(view, "post");
                assert_eq!(params.get("id"), Some(&"abc123".into()));
            }
            other => panic!("expected Navigate, got {other:?}"),
        }
    }

    #[test]
    fn router_invite_handler() {
        let mut router = UriRouter::new();
        router.register(Box::new(InviteHandler));

        let action = router
            .route_str("omnidea://invite?token=xyz789&relay=ws://localhost")
            .unwrap();

        match action {
            UriAction::Invite { code } => assert_eq!(code, "xyz789"),
            other => panic!("expected Invite, got {other:?}"),
        }
    }

    #[test]
    fn router_no_handler_returns_unknown() {
        let router = UriRouter::new();
        let uri = OmnideaUri::parse("omnidea://unknown/thing/123").unwrap();
        assert_eq!(router.route(&uri), UriAction::Unknown);
    }

    #[test]
    fn router_first_match_wins() {
        let mut router = UriRouter::new();
        router.register(Box::new(InviteHandler));
        router.register(Box::new(PlexusHandler));

        // InviteHandler won't match plexus URIs, so PlexusHandler handles it.
        let uri = OmnideaUri::parse("omnidea://plexus/post/abc").unwrap();
        let action = router.route(&uri);
        assert!(matches!(action, UriAction::Navigate { .. }));
    }

    #[test]
    fn router_handler_count() {
        let mut router = UriRouter::new();
        assert_eq!(router.handler_count(), 0);
        router.register(Box::new(PlexusHandler));
        assert_eq!(router.handler_count(), 1);
        router.register(Box::new(InviteHandler));
        assert_eq!(router.handler_count(), 2);
    }

    #[test]
    fn route_str_invalid_uri() {
        let router = UriRouter::new();
        assert!(router.route_str("https://google.com").is_err());
    }

    // -- LinkBuilder tests --

    #[test]
    fn link_builder_post() {
        let uri = LinkBuilder::post("abc123");
        assert_eq!(uri.to_uri(), "omnidea://plexus/post/abc123");
    }

    #[test]
    fn link_builder_design() {
        let uri = LinkBuilder::design("def456");
        assert_eq!(uri.to_uri(), "omnidea://luminaria/design/def456");
    }

    #[test]
    fn link_builder_community() {
        let uri = LinkBuilder::community("comm1");
        assert_eq!(uri.to_uri(), "omnidea://omny/community/comm1");
    }

    #[test]
    fn link_builder_profile() {
        let name = LinkBuilder::profile("sam");
        assert_eq!(name.domain(), "sam.idea");
    }

    #[test]
    fn link_builder_invite() {
        let uri = LinkBuilder::invite("ws://relay:8080", "tok123", "cpub_alice");
        assert!(uri.is_invite());
        assert_eq!(uri.params.get("relay"), Some(&"ws://relay:8080".into()));
        assert_eq!(uri.params.get("token"), Some(&"tok123".into()));
        assert_eq!(uri.params.get("inviter"), Some(&"cpub_alice".into()));
    }

    #[test]
    fn link_builder_generic_app() {
        let uri = LinkBuilder::app("tome", "document", "doc123");
        assert_eq!(uri.to_uri(), "omnidea://tome/document/doc123");
    }

    // -- Integration tests --

    #[test]
    fn end_to_end_link_share_and_route() {
        // 1. Build a shareable link.
        let uri = LinkBuilder::post("event_abc");

        // 2. Serialize to string (this is what gets shared).
        let shared = uri.to_uri();
        assert_eq!(shared, "omnidea://plexus/post/event_abc");

        // 3. Parse on the receiving end.
        let parsed = OmnideaUri::parse(&shared).unwrap();

        // 4. Route to a handler.
        let mut router = UriRouter::new();
        router.register(Box::new(PlexusHandler));
        let action = router.route(&parsed);

        match action {
            UriAction::Navigate { app, view, params } => {
                assert_eq!(app, "plexus");
                assert_eq!(view, "post");
                assert_eq!(params.get("id"), Some(&"event_abc".into()));
            }
            other => panic!("expected Navigate, got {other:?}"),
        }
    }

    #[test]
    fn end_to_end_globe_name_to_uri() {
        // Globe name → URI resolution path.
        let name = GlobeName::parse("sam.idea/portfolio").unwrap();
        assert_eq!(name.name, "sam");
        assert_eq!(name.path, Some("portfolio".into()));

        // In practice, resolve name → pubkey → fetch content.
        // Here we just verify the parsing round-trips.
        assert_eq!(name.to_address(), "sam.idea/portfolio");
    }
}
