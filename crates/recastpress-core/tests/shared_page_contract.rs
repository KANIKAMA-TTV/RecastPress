use recastpress_core::publication::{
    PrepareError, PreparedSite, PublicPage, PublishError, ReadError, publication_channel,
};
use std::{cell::Cell, sync::Arc};

fn page() -> PublicPage {
    PublicPage::new(
        200,
        "text/plain; charset=utf-8".into(),
        b"same public bytes".to_vec(),
    )
    .unwrap()
}

fn site(page: PublicPage) -> Result<PreparedSite, PrepareError> {
    PreparedSite::new([("/".into(), page)])
}

#[test]
fn reader_threads_and_response_clones_share_one_page_and_its_admitted_mime_buffer() {
    let mime = String::from("text/plain; charset=utf-8");
    let page = PublicPage::new(200, mime, b"public".to_vec()).unwrap();
    let admitted_mime = page.content_type().as_ptr();
    let (publisher, reader) = publication_channel();
    let base = publisher.revision().unwrap();
    let generation = publisher.publish(&base, site(page).unwrap()).unwrap();
    let (first, second) = std::thread::scope(|scope| {
        let one = reader.clone();
        let two = reader.clone();
        let one = scope.spawn(move || one.read("/").unwrap());
        let two = scope.spawn(move || two.read("/").unwrap());
        (one.join().unwrap(), two.join().unwrap())
    });
    let cloned = first.clone();
    assert!(std::ptr::eq(first.page(), second.page()));
    assert!(std::ptr::eq(first.page(), cloned.page()));
    for response in [&first, &second, &cloned] {
        assert_eq!(response.revision(), &generation);
        assert_eq!(response.page().content_type().as_ptr(), admitted_mime);
        assert_eq!(response.page().content_type(), "text/plain; charset=utf-8");
        assert_eq!(response.page().body(), b"public");
    }
    assert_eq!(reader.deliver(&first, PublicPage::status), Ok(200));
}

#[test]
fn shared_payload_does_not_make_independent_routes_or_replacements_the_same_page_or_revision() {
    let source = page();
    let (publisher, reader) = publication_channel();
    let initial = publisher.revision().unwrap();
    let generation = publisher
        .publish(
            &initial,
            PreparedSite::new([("/".into(), source.clone()), ("/b".into(), source.clone())])
                .unwrap(),
        )
        .unwrap();
    let old = reader.read("/").unwrap();
    let other_route = reader.read("/b").unwrap();
    assert!(!std::ptr::eq(old.page(), other_route.page()));
    assert!(Arc::ptr_eq(
        &old.page().shared_body(),
        &other_route.page().shared_body()
    ));
    let replacement = publisher
        .publish(&generation, site(source).unwrap())
        .unwrap();
    let current = reader.read("/").unwrap();
    assert!(!std::ptr::eq(old.page(), current.page()));
    assert!(Arc::ptr_eq(
        &old.page().shared_body(),
        &current.page().shared_body()
    ));
    assert_ne!(old.revision(), current.revision());
    assert_eq!(current.revision(), &replacement);
    let calls = Cell::new(0);
    assert_eq!(
        reader.deliver(&old, |_| calls.set(calls.get() + 1)),
        Err(ReadError::Stale)
    );
    assert_eq!(calls.get(), 0);
    // Inspectable old snapshot data was already supported; it is not authority.
    assert_eq!(old.page().body(), current.page().body());
    assert!(matches!(
        publisher.publish(&generation, site(page()).unwrap()),
        Err(PublishError::Conflict)
    ));
    assert_eq!(reader.deliver(&current, PublicPage::status), Ok(200));
}

#[test]
fn withdrawal_republication_and_cross_site_delivery_still_use_revision_authority() {
    let source = page();
    let (publisher, reader) = publication_channel();
    let (other_publisher, other_reader) = publication_channel();
    let first = publisher
        .publish(
            &publisher.revision().unwrap(),
            site(source.clone()).unwrap(),
        )
        .unwrap();
    other_publisher
        .publish(
            &other_publisher.revision().unwrap(),
            site(source.clone()).unwrap(),
        )
        .unwrap();
    let old = reader.read("/").unwrap();
    let retained = old.clone();
    let foreign = other_reader.read("/").unwrap();
    assert!(std::ptr::eq(old.page(), retained.page()));
    assert!(Arc::ptr_eq(
        &old.page().shared_body(),
        &foreign.page().shared_body()
    ));
    assert_eq!(
        reader.deliver(&foreign, |_| panic!("foreign callback ran")),
        Err::<(), _>(ReadError::Stale)
    );
    let withdrawn = publisher.withdraw(&first).unwrap();
    assert!(matches!(reader.read("/"), Err(ReadError::NotPublished)));
    assert_eq!(retained.page().body(), b"same public bytes");
    assert_eq!(
        reader.deliver(&retained, |_| panic!("withdrawn callback ran")),
        Err::<(), _>(ReadError::Stale)
    );
    let republished = publisher
        .publish(&withdrawn, site(source).unwrap())
        .unwrap();
    let current = reader.read("/").unwrap();
    assert_eq!(current.revision(), &republished);
    assert_eq!(reader.deliver(&current, PublicPage::status), Ok(200));
    assert_eq!(
        reader.deliver(&old, |_| panic!("old callback ran")),
        Err::<(), _>(ReadError::Stale)
    );
}
