use recastpress_core::publication::{
    PrepareError, PreparedSite, PublicPage, RasterImageKind, ReadError, publication_channel,
};

#[test]
fn declared_raster_signatures_are_exact_and_do_not_expand_the_legacy_constructor() {
    // These are signature fixtures, not claims of valid decodable image files.
    let cases = [
        (RasterImageKind::Png, b"\x89PNG\r\n\x1a\n".as_slice()),
        (RasterImageKind::Jpeg, b"\xff\xd8\xff".as_slice()),
        (
            RasterImageKind::WebP,
            b"RIFF\xff\0\x12\x34WEBPVP".as_slice(),
        ),
    ];
    for (kind, signature) in cases {
        for status in [200, 404, 410] {
            let page = PublicPage::new_image(status, kind, signature.to_vec()).unwrap();
            assert_eq!(page.status(), status);
            assert_eq!(page.content_type(), kind.content_type());
            assert_eq!(page.body(), signature);
        }
        for end in 0..signature.len() {
            assert_eq!(
                PublicPage::new_image(200, kind, signature[..end].to_vec()).unwrap_err(),
                PrepareError::InvalidImageSignature
            );
        }
        assert_eq!(
            PublicPage::new(200, kind.content_type().into(), signature.to_vec()).unwrap_err(),
            PrepareError::InvalidContentType
        );
        for other in [
            RasterImageKind::Png,
            RasterImageKind::Jpeg,
            RasterImageKind::WebP,
        ] {
            if other != kind {
                assert_eq!(
                    PublicPage::new_image(200, other, signature.to_vec()).unwrap_err(),
                    PrepareError::InvalidImageSignature
                );
            }
        }
    }
    assert_eq!(
        PublicPage::new_image(302, RasterImageKind::Png, Vec::new()).unwrap_err(),
        PrepareError::InvalidStatus
    );
    assert_eq!(
        PublicPage::new_image(200, RasterImageKind::Png, b"<html>image</html>".to_vec())
            .unwrap_err(),
        PrepareError::InvalidImageSignature
    );
}

#[test]
fn binary_images_use_the_existing_generation_and_withdrawal_boundary() {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(0..=255);
    let page = PublicPage::new_image(200, RasterImageKind::Png, bytes.clone()).unwrap();
    let (publisher, reader) = publication_channel();
    let revision = publisher
        .publish(
            &publisher.revision().unwrap(),
            PreparedSite::new([("/image.png?v=1".into(), page)]).unwrap(),
        )
        .unwrap();
    let response = reader.read("/image.png?v=1").unwrap();
    assert_eq!(
        reader
            .deliver(&response, |page| page.body().to_vec())
            .unwrap(),
        bytes
    );
    assert_eq!(
        reader.read("/image.png?v=2").unwrap_err(),
        ReadError::NotFound
    );
    publisher.withdraw(&revision).unwrap();
    assert_eq!(
        reader.deliver(&response, |_| ()).unwrap_err(),
        ReadError::Stale
    );
    assert_eq!(
        reader.read("/image.png?v=1").unwrap_err(),
        ReadError::NotPublished
    );
}
