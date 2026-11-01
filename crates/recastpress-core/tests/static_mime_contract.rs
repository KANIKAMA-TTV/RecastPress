use recastpress_core::publication::{
    OriginFormLocation, PrepareError, PublicPage, RasterImageKind, RedirectStatus,
};

const TEXT: [&str; 4] = [
    "text/html; charset=utf-8",
    "text/plain; charset=utf-8",
    "application/rss+xml; charset=utf-8",
    "application/atom+xml; charset=utf-8",
];

#[test]
fn text_raster_and_redirect_constructors_keep_their_exact_existing_mime_domains() {
    for status in [200, 404, 410] {
        for mime in TEXT {
            let page = PublicPage::new(status, mime.to_owned(), b"public".to_vec()).unwrap();
            assert_eq!(page.status(), status);
            assert_eq!(page.content_type(), mime);
            assert_eq!(page.body(), b"public");
            assert_eq!(page.location(), None);
        }
        for (kind, mime, signature) in [
            (
                RasterImageKind::Png,
                "image/png",
                b"\x89PNG\r\n\x1a\n".as_slice(),
            ),
            (
                RasterImageKind::Jpeg,
                "image/jpeg",
                b"\xff\xd8\xff".as_slice(),
            ),
            (
                RasterImageKind::WebP,
                "image/webp",
                b"RIFF\0\0\0\0WEBPVP".as_slice(),
            ),
        ] {
            let page = PublicPage::new_image(status, kind, signature.to_vec()).unwrap();
            assert_eq!(page.status(), status);
            assert_eq!(page.content_type(), mime);
            assert_eq!(page.body(), signature); // Signature fixture, not pixel decoding.
            assert_eq!(page.location(), None);
            assert_eq!(
                PublicPage::new(status, mime.to_owned(), signature.to_vec()).unwrap_err(),
                PrepareError::InvalidContentType
            );
        }
    }
    let plain = PublicPage::new(200, TEXT[1].to_owned(), Vec::new()).unwrap();
    for status in [
        RedirectStatus::MovedPermanently,
        RedirectStatus::Found,
        RedirectStatus::SeeOther,
        RedirectStatus::TemporaryRedirect,
        RedirectStatus::PermanentRedirect,
    ] {
        let page = PublicPage::new_redirect(
            status,
            OriginFormLocation::new("/destination?q=1".into()).unwrap(),
        );
        assert_eq!(page.status(), status.code());
        assert_eq!(page.content_type(), TEXT[1]);
        assert_eq!(page.location(), Some("/destination?q=1"));
        assert!(page.body().is_empty());
        // Both paths deliberately select the same canonical plain-text entry.
        assert_eq!(page.content_type().as_ptr(), plain.content_type().as_ptr());
    }
}

#[test]
fn mime_admission_does_not_trim_fold_expand_domains_or_change_status_error_priority() {
    for mime in [
        "",
        "text/plain",
        "application/json",
        "image/png",
        "TEXT/HTML; charset=utf-8",
        "text/html; charset=UTF-8",
        " text/html; charset=utf-8",
        "text/html; charset=utf-8 ",
        "text/html;charset=utf-8",
        "text/html; charset=utf-8; extra=1",
        "text/html; charset=utf-8\r\nx-private: marker",
        "text/plain; charset=utf-8\0",
    ] {
        assert_eq!(
            PublicPage::new(200, mime.to_owned(), b"public".to_vec()).unwrap_err(),
            PrepareError::InvalidContentType
        );
    }
    for status in [0, 199, 201, 204, 206, 301, 302, 400, 500] {
        assert_eq!(
            PublicPage::new(status, "invalid MIME".into(), Vec::new()).unwrap_err(),
            PrepareError::InvalidStatus
        );
        assert_eq!(
            PublicPage::new_image(status, RasterImageKind::Png, Vec::new()).unwrap_err(),
            PrepareError::InvalidStatus
        );
    }
    assert_eq!(
        PublicPage::new_image(200, RasterImageKind::Png, Vec::new()).unwrap_err(),
        PrepareError::InvalidImageSignature
    );
}

#[test]
fn admitted_mime_does_not_retain_caller_capacity_and_explicit_page_clones_share_it() {
    for mime in TEXT {
        let mut input = String::with_capacity(1024 * 1024);
        input.push_str(mime);
        assert!(input.capacity() >= 1024 * 1024);
        let caller_buffer = input.as_ptr();
        let page = PublicPage::new(200, input, b"body".to_vec()).unwrap();
        let admitted = page.content_type().as_ptr();
        assert_ne!(admitted, caller_buffer); // Compare only; never dereference a dropped pointer.
        let cloned = page.clone();
        let independently_admitted =
            PublicPage::new(404, mime.to_owned(), b"other".to_vec()).unwrap();
        assert_eq!(page.content_type(), mime);
        assert_eq!(cloned.content_type().as_ptr(), admitted);
        assert_eq!(independently_admitted.content_type().as_ptr(), admitted);
        assert_eq!(cloned.body(), b"body");
        assert_eq!(independently_admitted.status(), 404);
    }
}
