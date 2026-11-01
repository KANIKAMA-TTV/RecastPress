//! Startup-only validators for this server's sealed identity representations.
//! A digest is not a signature, authorization, monitor or publication lease.

use axum::http::HeaderValue;
use recastpress_core::publication::PublicPage;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub(crate) struct RepresentationTag {
    opaque: [u8; 64],
    header: HeaderValue,
}

impl RepresentationTag {
    pub(crate) fn from_page(page: &PublicPage) -> Self {
        let mut hash = Sha256::new();
        hash.update(b"recastpress-sealed-etag-v1\0");
        let status = page.status().to_be_bytes();
        for field in [
            status.as_slice(),
            page.content_type().as_bytes(),
            page.body(),
        ] {
            hash.update((field.len() as u64).to_be_bytes());
            hash.update(field);
        }
        let digest = hash.finalize();
        let alphabet = b"0123456789abcdef";
        let mut opaque = [0; 64];
        for (index, byte) in digest.iter().copied().enumerate() {
            opaque[index * 2] = alphabet[usize::from(byte >> 4)];
            opaque[index * 2 + 1] = alphabet[usize::from(byte & 15)];
        }
        let mut quoted = [b'"'; 66];
        quoted[1..65].copy_from_slice(&opaque);
        Self {
            opaque,
            header: HeaderValue::from_bytes(&quoted).expect("generated quoted ASCII digest"),
        }
    }

    pub(crate) fn header(&self) -> HeaderValue {
        self.header.clone()
    }

    pub(crate) fn opaque(&self) -> &[u8] {
        &self.opaque
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(status: u16, mime: &str, bytes: &[u8]) -> PublicPage {
        PublicPage::new(status, mime.into(), bytes.to_vec()).unwrap()
    }

    #[test]
    fn framing_matches_independent_sha256_and_generates_one_strong_quoted_tag() {
        let page = page(200, "text/plain; charset=utf-8", b"abc");
        let tag = RepresentationTag::from_page(&page);
        // Python hashlib: fixed domain, then u64BE length and raw bytes for
        // statusBE2, MIME and body. The digest is independent of SHA2's API.
        assert_eq!(
            tag.header().as_bytes(),
            b"\"72a6353ba83eaae657d4902e756bd5d3559f3a3de4733c5f95659203a6b2391d\""
        );
        assert_eq!(tag.opaque(), &tag.header().as_bytes()[1..65]);
        assert!(
            tag.opaque()
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        );
        assert_eq!(RepresentationTag::from_page(&page).header(), tag.header());
    }

    #[test]
    fn raw_body_mime_and_status_are_all_part_of_the_validator() {
        let baseline =
            RepresentationTag::from_page(&page(200, "text/plain; charset=utf-8", b"abc"));
        for changed in [
            page(200, "text/plain; charset=utf-8", b"abc\0"),
            page(200, "text/html; charset=utf-8", b"abc"),
            page(404, "text/plain; charset=utf-8", b"abc"),
            page(200, "text/plain; charset=utf-8", b""),
            page(200, "text/plain; charset=utf-8", b"\xff\0"),
        ] {
            assert_ne!(
                RepresentationTag::from_page(&changed).header(),
                baseline.header()
            );
        }
        let empty = page(200, "text/plain; charset=utf-8", b"");
        assert_eq!(
            RepresentationTag::from_page(&empty).header(),
            RepresentationTag::from_page(&empty.clone()).header()
        );
    }
}
