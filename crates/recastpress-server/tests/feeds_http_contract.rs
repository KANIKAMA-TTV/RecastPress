use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use recastpress_core::publication::{PreparedSite, PublicPage};
use recastpress_server::{
    feeds::{FeedDocument, FeedFormat, UtcTimestamp, render},
    public_router,
};
use tower::ServiceExt;

#[tokio::test]
async fn generated_feeds_enter_sealed_public_pages_with_mime_head_and_host_checks() {
    let mut feed = FeedDocument {
        id: "https://example.test/feed-id".into(),
        title: "Example feed".into(),
        home_url: "https://example.test/".into(),
        self_url: "https://example.test/feed/rss".into(),
        description: "Public prepared content".into(),
        author_name: "Example author".into(),
        updated: UtcTimestamp::new(2026, 10, 26, 0, 0, 0).unwrap(),
        entries: Vec::new(),
    };
    let mut pages = Vec::new();
    let mut expected = Vec::new();
    for (path, format) in [
        ("/feed/rss", FeedFormat::Rss2),
        ("/feed/atom", FeedFormat::Atom1),
    ] {
        feed.self_url = format!("https://example.test{path}");
        let bytes = render(&feed, format, 65536).unwrap();
        assert!(
            String::from_utf8_lossy(&bytes)
                .contains(&format!("rel=\"self\" href=\"{}\"", feed.self_url))
        );
        pages.push((
            path.into(),
            PublicPage::new(200, format.content_type().into(), bytes.clone()).unwrap(),
        ));
        expected.push((path, format.content_type(), bytes));
    }
    let app = public_router(Some(PreparedSite::new(pages).unwrap()), "example.test").unwrap();
    for (path, content_type, body) in expected {
        for method in ["GET", "HEAD"] {
            let request = Request::builder()
                .method(method)
                .uri(path)
                .header(header::HOST, "example.test")
                .body(Body::empty())
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], content_type);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()["x-content-type-options"], "nosniff");
            let received = to_bytes(response.into_body(), 65536).await.unwrap();
            if method == "HEAD" {
                assert!(received.is_empty());
            } else {
                assert_eq!(received.as_ref(), body);
            }
        }
        let wrong_host = Request::builder()
            .uri(path)
            .header(header::HOST, "other.test")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(wrong_host).await.unwrap().status(),
            StatusCode::MISDIRECTED_REQUEST
        );
    }
}
