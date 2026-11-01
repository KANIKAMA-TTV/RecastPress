use recastpress_core::publication::*;

fn page(body: &str) -> PublicPage {
    PublicPage::new(
        200,
        "text/html; charset=utf-8".into(),
        body.as_bytes().to_vec(),
    )
    .unwrap()
}

fn site(body: &str) -> PreparedSite {
    PreparedSite::new([
        ("/".into(), page(body)),
        ("/?q=rust".into(), page("search")),
    ])
    .unwrap()
}

#[test]
fn unpublished_then_atomic_generation_then_withdrawal() {
    let (publisher, reader) = publication_channel();
    assert_eq!(reader.read("/").unwrap_err(), ReadError::NotPublished);
    let initial = publisher.revision().unwrap();
    let first = publisher.publish(&initial, site("old")).unwrap();
    let old = reader.read("/").unwrap();
    assert_eq!(reader.deliver(&old, |p| p.body().to_vec()).unwrap(), b"old");
    let second = publisher.publish(&first, site("new")).unwrap();
    assert_eq!(
        reader.deliver(&old, |_| panic!("stale callback ran")),
        Err(ReadError::Stale)
    );
    let new = reader.read("/").unwrap();
    assert_eq!(new.page().body(), b"new");
    publisher.withdraw(&second).unwrap();
    assert_eq!(reader.read("/").unwrap_err(), ReadError::NotPublished);
    assert_eq!(reader.deliver(&new, |_| ()), Err(ReadError::Stale));
}

#[test]
fn concurrent_edit_and_cross_site_revision_are_rejected() {
    let (publisher, _) = publication_channel();
    let (other, _) = publication_channel();
    let initial = publisher.revision().unwrap();
    publisher.publish(&initial, site("first")).unwrap();
    assert_eq!(
        publisher.publish(&initial, site("lost update")),
        Err(PublishError::Conflict)
    );
    assert_eq!(
        publisher.withdraw(&other.revision().unwrap()),
        Err(PublishError::Conflict)
    );
}

#[test]
fn cross_site_response_cannot_be_delivered() {
    let (first, reader) = publication_channel();
    let (second, other) = publication_channel();
    first
        .publish(&first.revision().unwrap(), site("first"))
        .unwrap();
    second
        .publish(&second.revision().unwrap(), site("second"))
        .unwrap();
    let response = reader.read("/").unwrap();
    assert_eq!(other.deliver(&response, |_| ()), Err(ReadError::Stale));
}

#[test]
fn route_and_header_boundaries_reject_ambiguous_input() {
    for target in [
        "",
        "relative",
        "//evil.test/",
        "/\r\nInjected: x",
        "/foo bar",
        "/#fragment",
        "/\\evil",
    ] {
        assert!(!valid_target(target), "{target:?}");
    }
    assert!(valid_target("/?a=%20&a=+"));
    assert_eq!(
        PreparedSite::new([("/".into(), page("a")), ("/".into(), page("b"))]).unwrap_err(),
        PrepareError::DuplicateTarget
    );
    assert_eq!(
        PublicPage::new(302, "text/html; charset=utf-8".into(), vec![]).unwrap_err(),
        PrepareError::InvalidStatus
    );
    assert_eq!(
        PublicPage::new(200, "text/html\r\nx: y".into(), vec![]).unwrap_err(),
        PrepareError::InvalidContentType
    );
}

#[test]
fn query_variants_do_not_fall_back_to_unrelated_routes() {
    let (publisher, reader) = publication_channel();
    publisher
        .publish(&publisher.revision().unwrap(), site("home"))
        .unwrap();
    assert_eq!(reader.read("/?q=rust").unwrap().page().body(), b"search");
    assert_eq!(reader.read("/?q=php").unwrap_err(), ReadError::NotFound);
}

#[test]
fn withdrawal_waits_for_active_synchronous_delivery_then_rejects_old_output() {
    use std::{sync::mpsc, time::Duration};
    let (publisher, reader) = publication_channel();
    let base = publisher
        .publish(&publisher.revision().unwrap(), site("old"))
        .unwrap();
    let response = reader.read("/").unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (attempt_tx, attempt_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let delivery_reader = &reader;
        let delivery_response = &response;
        scope.spawn(move || {
            delivery_reader
                .deliver(delivery_response, |_| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                })
                .unwrap()
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        scope.spawn(|| {
            attempt_tx.send(()).unwrap();
            publisher.withdraw(&base).unwrap();
            done_tx.send(()).unwrap();
        });
        attempt_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        release_tx.send(()).unwrap();
        done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    assert_eq!(reader.deliver(&response, |_| ()), Err(ReadError::Stale));
}
