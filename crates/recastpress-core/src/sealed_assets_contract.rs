use super::*;

#[test]
fn named_assets_are_status200_exact_static_mimes_and_preserve_valid_utf8_bytes() {
    let css =
        b"\xef\xbb\xbf/* public */\r\n:root { --label: '\xe6\x97\xa5\xe6\x9c\xac'; }\n".to_vec();
    let js = "// public\r\nconst text = '日本🦀';\u{2028}\n"
        .as_bytes()
        .to_vec();
    for (page, expected, mime) in [
        (
            PublicPage::new_stylesheet(css.clone()).unwrap(),
            css,
            TEXT_CSS,
        ),
        (
            PublicPage::new_javascript(js.clone()).unwrap(),
            js,
            TEXT_JAVASCRIPT,
        ),
    ] {
        assert_eq!(page.status(), 200);
        assert_eq!(page.content_type(), mime);
        assert!(std::ptr::eq(page.content_type().as_ptr(), mime.as_ptr()));
        assert_eq!(page.body(), expected);
        assert!(page.location().is_none());
        let cloned = page.clone();
        assert!(std::ptr::eq(
            cloned.content_type().as_ptr(),
            page.content_type().as_ptr()
        ));
        assert!(std::ptr::eq(cloned.body().as_ptr(), page.body().as_ptr()));
    }
}

#[test]
fn utf8_admission_does_not_claim_css_or_javascript_syntax_validation() {
    for raw in [
        b"".as_slice(),
        b"\0",
        b"not valid CSS or JS !!!",
        b"</script><script>literal source",
    ] {
        assert_eq!(
            PublicPage::new_stylesheet(raw.to_vec()).unwrap().body(),
            raw
        );
        assert_eq!(
            PublicPage::new_javascript(raw.to_vec()).unwrap().body(),
            raw
        );
    }
    for raw in [
        b"\xff".as_slice(),
        b"\xc0\xaf",
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"end\xe2\x82",
    ] {
        assert_eq!(
            PublicPage::new_stylesheet(raw.to_vec()).unwrap_err(),
            PrepareError::InvalidTextAssetEncoding
        );
        assert_eq!(
            PublicPage::new_javascript(raw.to_vec()).unwrap_err(),
            PrepareError::InvalidTextAssetEncoding
        );
    }
}

#[test]
fn legacy_constructor_mime_and_status_domain_remains_unchanged() {
    for status in [200, 404, 410] {
        for mime in [TEXT_HTML, TEXT_PLAIN, TEXT_RSS, TEXT_ATOM] {
            assert!(PublicPage::new(status, mime.into(), b"legacy".to_vec()).is_ok());
        }
        for mime in [
            TEXT_CSS,
            TEXT_JAVASCRIPT,
            "text/css",
            "text/javascript",
            "application/javascript",
            "image/png",
            "TEXT/CSS; charset=utf-8",
            " text/css; charset=utf-8",
        ] {
            assert_eq!(
                PublicPage::new(status, mime.into(), vec![]).unwrap_err(),
                PrepareError::InvalidContentType
            );
        }
    }
    assert_eq!(
        PublicPage::new(201, TEXT_CSS.into(), vec![]).unwrap_err(),
        PrepareError::InvalidStatus
    );
    for (kind, raw) in [
        (RasterImageKind::Png, b"\x89PNG\r\n\x1a\n".as_slice()),
        (RasterImageKind::Jpeg, b"\xff\xd8\xff".as_slice()),
        (RasterImageKind::WebP, b"RIFF\x06\0\0\0WEBPVP".as_slice()),
    ] {
        assert!(PublicPage::new_image(200, kind, raw.to_vec()).is_ok());
    }
}

#[test]
fn fixed_host_html_css_js_selection_has_shared_generation_and_only_selected_owners() {
    let css = PublicPage::new_stylesheet(b"h1 { color: teal; }\n".to_vec()).unwrap();
    let js = PublicPage::new_javascript(
        b"document.documentElement.dataset.fixture = 'public';\n".to_vec(),
    )
    .unwrap();
    let weak_css = Arc::downgrade(&css.body);
    let prepared = PreparedSite::new([
        ("/".into(), PublicPage::new(200, TEXT_HTML.into(), b"<!doctype html><link rel=stylesheet href='/theme.css?v=1'><script src='/theme.js?v=1'></script>".to_vec()).unwrap()),
        ("/theme.css?v=1".into(), css), ("/theme.js?v=1".into(), js),
    ]).unwrap();
    let (publisher, reader) = publication_channel();
    let revision = publisher
        .publish(&publisher.revision().unwrap(), prepared)
        .unwrap();
    let selected = reader
        .read_selected(
            &["/", "/theme.css?v=1", "/theme.js?v=1"],
            SelectionLimits::new(3, 64),
        )
        .unwrap();
    assert_eq!(selected.revision(), &revision);
    assert_eq!(
        selected
            .pages()
            .map(PublicPage::content_type)
            .collect::<Vec<_>>(),
        [TEXT_HTML, TEXT_CSS, TEXT_JAVASCRIPT]
    );
    assert!(reader.read("/theme.css?v=2").is_err());
    let cloned = selected.try_clone().unwrap();
    publisher.withdraw(&revision).unwrap();
    assert_eq!(
        reader.deliver_selected(&selected, |_| ()),
        Err(ReadError::Stale)
    );
    drop(selected);
    assert!(weak_css.upgrade().is_some());
    drop(cloned);
    assert!(weak_css.upgrade().is_none());
}
