use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

fn page(raw: &[u8]) -> PublicPage {
    PublicPage::new(200, "text/plain; charset=utf-8".into(), raw.to_vec()).unwrap()
}
fn site(label: &str) -> PreparedSite {
    PreparedSite::new([
        (
            "/article?b=2&a=1".into(),
            page(format!("{label} article").as_bytes()),
        ),
        (
            "/navigation".into(),
            page(format!("{label} navigation").as_bytes()),
        ),
        ("/other".into(), page(b"unselected")),
    ])
    .unwrap()
}
fn limits() -> SelectionLimits {
    SelectionLimits::new(3, 8192)
}
const TARGETS: [&str; 2] = ["/article?b=2&a=1", "/navigation"];
fn render(selection: &PublicSelection) -> Vec<u8> {
    let mut result = Vec::new();
    for page in selection.pages() {
        result.extend_from_slice(page.body());
        result.push(b'\n');
    }
    result
}

#[test]
fn fixed_article_navigation_consumer_selects_one_generation_but_separate_reads_can_mix() {
    use std::sync::mpsc;
    use std::time::Duration;
    let deadline = Duration::from_secs(5);
    let (publisher, reader) = publication_channel();
    let revision = publisher
        .publish(&publisher.revision().unwrap(), site("A"))
        .unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let worker_reader = reader.clone();
    let worker = std::thread::spawn(move || {
        let result = (|| {
            let article = worker_reader.read(TARGETS[0])?;
            ready_tx.send(()).map_err(|_| ReadError::Unavailable)?;
            release_rx
                .recv_timeout(deadline)
                .map_err(|_| ReadError::Unavailable)?;
            let navigation = worker_reader.read(TARGETS[1])?;
            Ok::<_, ReadError>((article, navigation))
        })();
        let _ = completed_tx.send(result);
    });
    let ready = ready_rx.recv_timeout(deadline);
    let published = ready
        .is_ok()
        .then(|| publisher.publish(&revision, site("B")));
    // Release even on a failed observation/publication before reporting it.
    let _ = release_tx.send(());
    let completed = completed_rx.recv_timeout(deadline);
    if completed.is_ok() {
        worker.join().unwrap();
    }
    assert!(ready.is_ok());
    assert!(published.unwrap().is_ok());
    let (article, navigation) = completed.unwrap().unwrap();
    assert_eq!(article.page().body(), b"A article");
    assert_eq!(navigation.page().body(), b"B navigation");
    assert_ne!(article.revision(), navigation.revision());
    let selection = reader.read_selected(&TARGETS, limits()).unwrap();
    assert_eq!(selection.len(), 2);
    assert!(!selection.is_empty());
    assert_eq!(selection.pages().len(), 2);
    assert_eq!(
        reader.deliver_selected(&selection, render).unwrap(),
        b"B article\nB navigation\n"
    );
}

#[test]
fn replace_withdraw_and_cross_site_are_stale_without_callback_but_old_values_are_inspectable() {
    let (publisher, reader) = publication_channel();
    let a = publisher
        .publish(&publisher.revision().unwrap(), site("A"))
        .unwrap();
    let old = reader.read_selected(&TARGETS, limits()).unwrap();
    assert_eq!(old.revision(), &a);
    let clone = old.try_clone().unwrap();
    let b = publisher.publish(&a, site("B")).unwrap();
    let called = AtomicBool::new(false);
    assert_eq!(
        reader.deliver_selected(&old, |_| called.store(true, Ordering::SeqCst)),
        Err(ReadError::Stale)
    );
    assert!(!called.load(Ordering::SeqCst));
    assert_eq!(render(&clone), b"A article\nA navigation\n");
    let current = reader.read_selected(&TARGETS, limits()).unwrap();
    let (other_publisher, other_reader) = publication_channel();
    other_publisher
        .publish(&other_publisher.revision().unwrap(), site("B"))
        .unwrap();
    assert_eq!(
        other_reader.deliver_selected(&current, |_| ()),
        Err(ReadError::Stale)
    );
    publisher.withdraw(&b).unwrap();
    assert_eq!(
        reader.deliver_selected(&current, |_| called.store(true, Ordering::SeqCst)),
        Err(ReadError::Stale)
    );
    assert!(!called.load(Ordering::SeqCst));
}

#[test]
fn exact_caps_count_utf8_and_duplicates_preserve_order_and_share_only_selected_pages() {
    let (publisher, reader) = publication_channel();
    let prepared = PreparedSite::new([
        ("/日本".into(), page(b"unicode")),
        ("/b".into(), page(b"second")),
        ("/unselected".into(), page(b"drop")),
    ])
    .unwrap();
    let selected_weak = Arc::downgrade(prepared.pages.get("/日本").unwrap());
    let other_weak = Arc::downgrade(prepared.pages.get("/unselected").unwrap());
    let revision = publisher
        .publish(&publisher.revision().unwrap(), prepared)
        .unwrap();
    let targets = ["/b", "/日本", "/日本"];
    let bytes: usize = targets.iter().map(|s| s.len()).sum();
    assert_eq!(
        reader
            .read_selected(&targets, SelectionLimits::new(2, bytes))
            .unwrap_err(),
        SelectionError::Limits
    );
    assert_eq!(
        reader
            .read_selected(&targets, SelectionLimits::new(3, bytes - 1))
            .unwrap_err(),
        SelectionError::Limits
    );
    let selection = reader
        .read_selected(&targets, SelectionLimits::new(3, bytes))
        .unwrap();
    let owners: Vec<_> = selection.pages().collect();
    assert_eq!(owners[0].body(), b"second");
    assert!(std::ptr::eq(owners[1], owners[2]));
    let cloned = selection.try_clone().unwrap();
    publisher.withdraw(&revision).unwrap();
    assert!(other_weak.upgrade().is_none());
    assert!(selected_weak.upgrade().is_some());
    drop(selection);
    assert!(selected_weak.upgrade().is_some());
    drop(cloned);
    assert!(selected_weak.upgrade().is_none());
}

#[test]
fn validation_caps_missing_and_unpublished_are_all_or_nothing_with_query_exactness() {
    let (publisher, reader) = publication_channel();
    assert_eq!(
        reader.read_selected(&[], limits()).unwrap_err(),
        SelectionError::InvalidSelection
    );
    assert_eq!(
        reader
            .read_selected(&["invalid"], SelectionLimits::new(0, 0))
            .unwrap_err(),
        SelectionError::Limits
    );
    assert_eq!(
        reader.read_selected(&["invalid"], limits()).unwrap_err(),
        SelectionError::InvalidSelection
    );
    assert_eq!(
        reader.read_selected(&TARGETS, limits()).unwrap_err(),
        SelectionError::Read(ReadError::NotPublished)
    );
    publisher
        .publish(&publisher.revision().unwrap(), site("A"))
        .unwrap();
    assert_eq!(
        reader
            .read_selected(&[TARGETS[0], "/missing"], limits())
            .unwrap_err(),
        SelectionError::Read(ReadError::NotFound)
    );
    assert_eq!(
        reader
            .read_selected(&["/article?a=1&b=2"], limits())
            .unwrap_err(),
        SelectionError::Read(ReadError::NotFound)
    );
    for invalid in ["//host", "/a#fragment", "/a b", "/a\\b"] {
        assert_eq!(
            reader.read_selected(&[invalid], limits()).unwrap_err(),
            SelectionError::InvalidSelection
        );
    }
}

#[test]
fn poisoned_state_fails_closed_and_input_refusals_still_precede_the_lock() {
    let (publisher, reader) = publication_channel();
    publisher
        .publish(&publisher.revision().unwrap(), site("A"))
        .unwrap();
    let selected = reader.read_selected(&TARGETS, limits()).unwrap();
    let state = Arc::clone(&reader.0);
    let _ = std::thread::spawn(move || {
        let _held = state.write().unwrap();
        panic!("fixed poison fixture");
    })
    .join();
    assert_eq!(
        reader.read_selected(&TARGETS, limits()).unwrap_err(),
        SelectionError::Read(ReadError::Unavailable)
    );
    assert_eq!(
        reader.read_selected(&[], limits()).unwrap_err(),
        SelectionError::InvalidSelection
    );
    assert_eq!(
        reader
            .read_selected(&TARGETS, SelectionLimits::new(0, 0))
            .unwrap_err(),
        SelectionError::Limits
    );
    assert_eq!(
        reader.deliver_selected(&selected, |_| panic!("must not deliver")),
        Err(ReadError::Unavailable)
    );
}

#[test]
fn group_delivery_holds_read_lock_until_callback_returns_and_then_publish_completes() {
    use std::sync::{TryLockError, mpsc};
    use std::time::Duration;
    let deadline = Duration::from_secs(5);
    let (publisher, reader) = publication_channel();
    let base = publisher
        .publish(&publisher.revision().unwrap(), site("A"))
        .unwrap();
    let selected = reader.read_selected(&TARGETS, limits()).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (delivered_tx, delivered_rx) = mpsc::channel();
    let callback_reader = reader.clone();
    let callback_state = Arc::clone(&reader.0);
    let delivery = std::thread::spawn(move || {
        let result = callback_reader.deliver_selected(&selected, |group| {
            // The production implementation adds no test hook: this private
            // same-state probe proves the callback runs while a read lock lives.
            assert!(matches!(
                callback_state.try_write(),
                Err(TryLockError::WouldBlock)
            ));
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(deadline).unwrap();
            render(group)
        });
        delivered_tx.send(result).unwrap();
    });
    let entered = entered_rx.recv_timeout(deadline);
    if entered.is_err() {
        let _ = release_tx.send(());
        if delivered_rx.recv_timeout(deadline).is_ok() {
            delivery.join().unwrap();
        }
        panic!("delivery callback did not enter within finite fixture deadline");
    }
    let (attempt_tx, attempt_rx) = mpsc::channel();
    let (published_tx, published_rx) = mpsc::channel();
    let publication = std::thread::spawn(move || {
        attempt_tx.send(()).unwrap();
        let result = publisher.publish(&base, site("B"));
        published_tx.send(result).unwrap();
    });
    let attempted = attempt_rx.recv_timeout(deadline);
    // Source lock ownership, not this timeout alone, is the atomicity evidence.
    // The timeout provides a finite observation while the callback is held.
    let before_release = published_rx.recv_timeout(Duration::from_millis(20));
    let still_read_locked = matches!(reader.0.try_write(), Err(TryLockError::WouldBlock));
    let _ = release_tx.send(());
    let delivered = delivered_rx.recv_timeout(deadline);
    let published = published_rx.recv_timeout(deadline);
    // Join only after the corresponding completion message, so a failing
    // timeout observation does not turn into an unbounded test-thread join.
    if delivered.is_ok() {
        delivery.join().unwrap();
    }
    if published.is_ok() {
        publication.join().unwrap();
    }
    assert!(attempted.is_ok());
    assert!(matches!(
        before_release,
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(still_read_locked);
    assert_eq!(delivered.unwrap().unwrap(), b"A article\nA navigation\n");
    assert!(published.unwrap().is_ok());
    let current = reader.read_selected(&TARGETS, limits()).unwrap();
    assert_eq!(render(&current), b"B article\nB navigation\n");
}
