use recastpress_core::publication::{
    MAX_TARGET_BYTES, OriginFormLocation, PrepareError, PreparedSite, PublicPage, RasterImageKind,
    ReadError, RedirectStatus, publication_channel,
};

#[test]
fn location_admission_preserves_supported_bytes_and_rejects_authority_and_header_forms() {
    for value in [
        "/",
        "/a/../b?x=1&x=2+a",
        "/%e6%97%a5?x=%41%3F",
        "/!$&'()*+,;=:@~_-./??",
    ] {
        let input = value.to_owned();
        let location = OriginFormLocation::new(input.clone()).unwrap();
        assert_eq!(location.as_str(), input);
        assert_eq!(location.clone().as_str(), input);
    }
    for value in [
        "",
        "relative",
        "https://example.test/",
        "//evil.test/",
        "/\\evil.test/",
        "\\evil.test",
        "?query",
        "#fragment",
        "/#fragment",
        "/a b",
        "/日本語",
        "/<x>",
        "/\"x",
        "/[x]",
        "/%",
        "/%1",
        "/%xy",
        "/%00",
        "/%0d%0A",
        "/%20",
        "/%7F",
        "/%2f%2Fevil",
        "/%5cfoo",
        "/a?x=%2f",
    ] {
        assert_eq!(
            OriginFormLocation::new(value.into()).unwrap_err(),
            PrepareError::InvalidLocation,
            "{value:?}"
        );
    }
    for byte in (0..=32u8).chain([127]) {
        let value = format!("/x{}y", char::from(byte));
        assert_eq!(
            OriginFormLocation::new(value).unwrap_err(),
            PrepareError::InvalidLocation
        );
    }
    let boundary = format!("/{}", "a".repeat(MAX_TARGET_BYTES - 1));
    assert_eq!(
        OriginFormLocation::new(boundary.clone()).unwrap().as_str(),
        boundary
    );
    assert_eq!(
        OriginFormLocation::new(format!("{boundary}a")).unwrap_err(),
        PrepareError::InvalidLocation
    );
}

#[test]
fn redirect_statuses_and_location_are_available_only_through_the_typed_constructor() {
    for code in [301, 302, 303, 307, 308] {
        let status = RedirectStatus::try_from(code).unwrap();
        let page =
            PublicPage::new_redirect(status, OriginFormLocation::new("/next?q=1".into()).unwrap());
        assert_eq!(page.status(), code);
        assert_eq!(page.location(), Some("/next?q=1"));
        assert_eq!(page.body(), b"");
        assert_eq!(page.content_type(), "text/plain; charset=utf-8");
        assert_eq!(
            PublicPage::new(code, "text/plain; charset=utf-8".into(), vec![]).unwrap_err(),
            PrepareError::InvalidStatus
        );
        assert_eq!(
            PublicPage::new_image(code, RasterImageKind::Png, b"\x89PNG\r\n\x1a\n".to_vec())
                .unwrap_err(),
            PrepareError::InvalidStatus
        );
    }
    for code in [0, 200, 201, 300, 304, 305, 306, 309, 404, 410, 65535] {
        assert_eq!(
            RedirectStatus::try_from(code).unwrap_err(),
            PrepareError::InvalidStatus
        );
    }
    let text = PublicPage::new(200, "text/plain; charset=utf-8".into(), b"body".to_vec()).unwrap();
    assert_eq!(text.location(), None);
}

#[test]
fn redirects_obey_generation_replacement_and_withdrawal_without_resolving_destinations() {
    let (publisher, reader) = publication_channel();
    let make_site = |destination: &str| {
        PreparedSite::new([(
            "/old".into(),
            PublicPage::new_redirect(
                RedirectStatus::PermanentRedirect,
                OriginFormLocation::new(destination.into()).unwrap(),
            ),
        )])
        .unwrap()
    };
    let revision = publisher
        .publish(&publisher.revision().unwrap(), make_site("/not-prepared"))
        .unwrap();
    let old = reader.read("/old").unwrap();
    assert_eq!(
        reader
            .deliver(&old, |page| page.location().unwrap().to_owned())
            .unwrap(),
        "/not-prepared"
    );
    assert_eq!(
        reader.read("/not-prepared").unwrap_err(),
        ReadError::NotFound
    );
    let revision = publisher.publish(&revision, make_site("/other")).unwrap();
    assert_eq!(reader.deliver(&old, |_| ()).unwrap_err(), ReadError::Stale);
    let new = reader.read("/old").unwrap();
    publisher.withdraw(&revision).unwrap();
    assert_eq!(reader.deliver(&new, |_| ()).unwrap_err(), ReadError::Stale);
    assert_eq!(reader.read("/old").unwrap_err(), ReadError::NotPublished);
}
