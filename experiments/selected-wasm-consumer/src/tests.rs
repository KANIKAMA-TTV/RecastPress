use super::*;
use recastpress_core::publication::{Publisher, Revision};
use std::{
    sync::{Arc, Weak},
    thread::JoinHandle,
};

// The import does not write publication state. A separate owned fixture thread
// waits for actual guest entry, writes once and then acknowledges that operation.
enum Change {
    Publish(&'static str),
    Withdraw,
}
struct Mutation {
    done: mpsc::Receiver<(Publisher, Result<Revision, ProbeError>)>,
    worker: Option<JoinHandle<()>>,
}
impl Mutation {
    fn spawn(publisher: Publisher, base: Revision, change: Change) -> (Self, Checkpoint) {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (ack_tx, ack_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = (|| {
                entered_rx
                    .recv_timeout(DEADLINE)
                    .map_err(|_| ProbeError::Checkpoint)?;
                let revision = match change {
                    Change::Publish(label) => publisher.publish(&base, site(label)),
                    Change::Withdraw => publisher.withdraw(&base),
                }
                .map_err(|_| ProbeError::Selection)?;
                ack_tx.send(()).map_err(|_| ProbeError::Checkpoint)?;
                Ok(revision)
            })();
            let _ = done_tx.send((publisher, result));
        });
        (
            Self {
                done: done_rx,
                worker: Some(worker),
            },
            Checkpoint::Controlled {
                entered: entered_tx,
                ack: ack_rx,
            },
        )
    }
    fn finish(mut self) -> Result<(Publisher, Revision), ProbeError> {
        // Completion is observed before join, so a regression does not convert
        // a timed-out fixture into an unconditional unbounded thread join.
        let (publisher, result) = self
            .done
            .recv_timeout(DEADLINE)
            .map_err(|_| ProbeError::WorkerUnconfirmed)?;
        self.worker
            .take()
            .ok_or(ProbeError::Profile)?
            .join()
            .map_err(|_| ProbeError::WorkerUnconfirmed)?;
        Ok((publisher, result?))
    }
}

fn published(label: &str) -> (Publisher, PublicReader, Revision) {
    let (publisher, reader) = publication::publication_channel();
    let revision = publisher
        .publish(&publisher.revision().unwrap(), site(label))
        .unwrap();
    (publisher, reader, revision)
}
fn expected(label: &str) -> Vec<u8> {
    format!("<article>{label} article</article><nav>{label} navigation</nav>").into_bytes()
}

#[test]
fn current_a_selection_runs_guest_after_read_lock_and_delivers_only_owned_post_return_complete_html()
 {
    assert_eq!(observe_current().unwrap(), expected("A").len());
    let runtime = Runtime::compile(Mode::Normal).unwrap();
    let caps = limits(4096, 8192, 10000);
    let (_publisher, reader, _) = published("A");
    let selection = select(&reader, caps).unwrap();
    let rendered = render_selected(&runtime, &selection, caps, Checkpoint::None).unwrap();
    assert_eq!(rendered.bytes, expected("A"));
    assert_eq!(rendered.allocations, 2);
    assert_eq!(rendered.counter, 1);
    let calls = AtomicU64::new(0);
    assert_eq!(
        deliver(&reader, &selection, rendered, &calls).unwrap(),
        expected("A")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.stores.load(Ordering::SeqCst), 1);
}

#[test]
fn real_guest_checkpoint_allows_b_publication_then_blocks_a_delivery_and_fresh_b_succeeds() {
    let runtime = Runtime::compile(Mode::Normal).unwrap();
    let caps = limits(4096, 8192, 10000);
    let (publisher, reader, a) = published("A");
    let selection = select(&reader, caps).unwrap();
    let (worker, checkpoint) = Mutation::spawn(publisher, a, Change::Publish("B"));
    let rendered = render_selected(&runtime, &selection, caps, checkpoint);
    let completion = worker.finish(); // Always finish worker before reporting render failure.
    let (_publisher, b) = completion.unwrap();
    let rendered = rendered.unwrap();
    assert_eq!(rendered.bytes, expected("A"));
    let copied_old = rendered.bytes.clone();
    let calls = AtomicU64::new(0);
    assert_eq!(
        deliver(&reader, &selection, rendered, &calls),
        Err(ProbeError::Stale)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(copied_old, expected("A"));
    let fresh = select(&reader, caps).unwrap();
    assert_eq!(fresh.revision(), &b);
    let new = render_selected(&runtime, &fresh, caps, Checkpoint::None).unwrap();
    assert_eq!(new.counter, 1);
    assert_eq!(
        deliver(&reader, &fresh, new, &calls).unwrap(),
        expected("B")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn real_guest_checkpoint_withdrawal_is_stale_and_explicit_new_c_selection_restores_delivery() {
    let runtime = Runtime::compile(Mode::Normal).unwrap();
    let caps = limits(4096, 8192, 10000);
    let (publisher, reader, a) = published("A");
    let selection = select(&reader, caps).unwrap();
    let (worker, checkpoint) = Mutation::spawn(publisher, a, Change::Withdraw);
    let rendered = render_selected(&runtime, &selection, caps, checkpoint);
    let completion = worker.finish();
    let (publisher, withdrawn) = completion.unwrap();
    let calls = AtomicU64::new(0);
    assert_eq!(
        deliver(&reader, &selection, rendered.unwrap(), &calls),
        Err(ProbeError::Stale)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let c = publisher.publish(&withdrawn, site("C")).unwrap();
    let fresh = select(&reader, caps).unwrap();
    assert_eq!(fresh.revision(), &c);
    let new = render_selected(&runtime, &fresh, caps, Checkpoint::None).unwrap();
    assert_eq!(
        deliver(&reader, &fresh, new, &calls).unwrap(),
        expected("C")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        reader.deliver_selected(&selection, |_| panic!("old generation cannot deliver")),
        Err(ReadError::Stale)
    );
}

#[test]
fn selected_body_owners_release_after_selection_drop_while_copied_html_survives() {
    let runtime = Runtime::compile(Mode::Normal).unwrap();
    let caps = limits(4096, 8192, 10000);
    let article = page(b"A article");
    let navigation = page(b"A navigation");
    let other = page(b"unselected");
    let article_weak: Weak<[u8]> = Arc::downgrade(&article.shared_body());
    let navigation_weak = Arc::downgrade(&navigation.shared_body());
    let other_weak = Arc::downgrade(&other.shared_body());
    let (publisher, reader) = publication::publication_channel();
    let a = publisher
        .publish(
            &publisher.revision().unwrap(),
            PreparedSite::new([
                (TARGETS[0].into(), article),
                (TARGETS[1].into(), navigation),
                ("/other".into(), other),
            ])
            .unwrap(),
        )
        .unwrap();
    let selection = select(&reader, caps).unwrap();
    let rendered = render_selected(&runtime, &selection, caps, Checkpoint::None).unwrap();
    publisher.publish(&a, site("B")).unwrap();
    assert!(other_weak.upgrade().is_none());
    assert!(article_weak.upgrade().is_some());
    assert!(navigation_weak.upgrade().is_some());
    drop(selection);
    assert!(article_weak.upgrade().is_none());
    assert!(navigation_weak.upgrade().is_none());
    assert_eq!(rendered.bytes, expected("A")); // Independent bytes, not Page ownership.
}

#[test]
fn mandatory_caps_and_invalid_inputs_fail_before_store_and_two_input_arena_accepts_exact_4096() {
    let runtime = Runtime::compile(Mode::Normal).unwrap();
    let caps = limits(4096, 8192, 10000);
    let (_publisher, reader, _) = published("A");
    let mut count_cap = caps;
    count_cap.selection = SelectionLimits::new(1, 100);
    assert!(select(&reader, count_cap).is_err());
    let mut target_cap = caps;
    target_cap.selection =
        SelectionLimits::new(2, TARGETS.iter().map(|s| s.len()).sum::<usize>() - 1);
    assert!(select(&reader, target_cap).is_err());
    assert_eq!(runtime.stores.load(Ordering::SeqCst), 0);
    let too_long = "x".repeat(4097);
    assert_eq!(
        runtime
            .fresh(&too_long, "", caps, Checkpoint::None)
            .unwrap_err(),
        ProbeError::InputLimit
    );
    let (invalid_publisher, invalid_reader) = publication::publication_channel();
    invalid_publisher
        .publish(
            &invalid_publisher.revision().unwrap(),
            PreparedSite::new([
                (TARGETS[0].into(), page(b"\xff")),
                (TARGETS[1].into(), page(b"public navigation")),
            ])
            .unwrap(),
        )
        .unwrap();
    let invalid_selection = select(&invalid_reader, caps).unwrap();
    assert_eq!(
        render_selected(&runtime, &invalid_selection, caps, Checkpoint::None).unwrap_err(),
        ProbeError::InvalidUtf8
    );
    assert_eq!(runtime.stores.load(Ordering::SeqCst), 0);
    for (article, navigation) in [("", ""), ("", "日本😀"), ("\0日本", ""), ("A", "\0B")] {
        let rendered = runtime
            .fresh(article, navigation, caps, Checkpoint::None)
            .unwrap();
        assert_eq!(
            rendered.bytes,
            format!("<article>{article}</article><nav>{navigation}</nav>").as_bytes()
        );
    }
    let first = "a".repeat(2048);
    let second = "b".repeat(2048);
    let mut exact_output = caps;
    exact_output.output = 4096 + 30;
    let rendered = runtime
        .fresh(&first, &second, exact_output, Checkpoint::None)
        .unwrap();
    assert_eq!(rendered.bytes.len(), 4126);
    let mut short_input = caps;
    short_input.input = 4095;
    let before = runtime.stores.load(Ordering::SeqCst);
    assert_eq!(
        runtime
            .fresh(&first, &second, short_input, Checkpoint::None)
            .unwrap_err(),
        ProbeError::InputLimit
    );
    assert_eq!(runtime.stores.load(Ordering::SeqCst), before);
    let mut short_output = caps;
    short_output.output = 4125;
    assert_eq!(
        runtime
            .fresh(&first, &second, short_output, Checkpoint::None)
            .unwrap_err(),
        ProbeError::OutputLimit
    );
    let mut memory = caps;
    memory.memory = 65535;
    assert_eq!(
        runtime
            .fresh("a", "b", memory, Checkpoint::None)
            .unwrap_err(),
        ProbeError::MemoryLimit
    );
}

#[test]
fn successful_call_refusals_still_post_return_and_fuel_failures_have_closed_phase_causes() {
    let caps = limits(4096, 8192, 10000);
    for (cap, copy_fail, expected) in [
        (0, false, ProbeError::OutputLimit),
        (8192, true, ProbeError::Allocation),
    ] {
        let runtime = Runtime::compile(Mode::Normal).unwrap();
        let (mut store, instance) = runtime.session(caps, Checkpoint::None).unwrap();
        assert_eq!(
            render(&mut store, &instance, "a", "b", cap, copy_fail).unwrap_err(),
            expected
        );
        assert_eq!(scalar(&mut store, &instance, "postcalls").unwrap(), 1);
    }
    let runtime = Runtime::compile(Mode::InvalidUtf8).unwrap();
    let (mut store, instance) = runtime.session(caps, Checkpoint::None).unwrap();
    assert_eq!(
        render(&mut store, &instance, "a", "b", 8192, false).unwrap_err(),
        ProbeError::InvalidUtf8
    );
    assert_eq!(scalar(&mut store, &instance, "postcalls").unwrap(), 1);
    for mode in [Mode::StartLoop, Mode::ReallocLoop, Mode::RenderLoop] {
        assert_eq!(
            Runtime::compile(mode)
                .unwrap()
                .fresh("a", "b", caps, Checkpoint::None)
                .unwrap_err(),
            ProbeError::Fuel
        );
    }
    for (mode, cap) in [
        (Mode::PostLoop, 8192),
        (Mode::PostLoop, 0),
        (Mode::InvalidUtf8PostLoop, 8192),
    ] {
        let mut output_limit = caps;
        output_limit.output = cap;
        assert_eq!(
            Runtime::compile(mode)
                .unwrap()
                .fresh("a", "b", output_limit, Checkpoint::None)
                .unwrap_err(),
            ProbeError::PostFuel
        );
    }
    // An import acknowledgement that cannot arrive is a typed error, not false/current success.
    let (entered, observed_entry) = mpsc::channel();
    let (ack_sender, ack) = mpsc::channel();
    drop(ack_sender);
    assert_eq!(
        Runtime::compile(Mode::Normal)
            .unwrap()
            .fresh("a", "b", caps, Checkpoint::Controlled { entered, ack })
            .unwrap_err(),
        ProbeError::Checkpoint
    );
    observed_entry.recv_timeout(DEADLINE).unwrap();
}

#[test]
fn same_instance_counter_one_then_two_is_control_and_fresh_instance_always_resets_to_one() {
    let runtime = Runtime::compile(Mode::Normal).unwrap();
    let caps = limits(4096, 8192, 10000);
    let (mut store, instance) = runtime.session(caps, Checkpoint::None).unwrap();
    let first = render(&mut store, &instance, "first", "nav", 8192, false).unwrap();
    let second = render(&mut store, &instance, "second", "nav", 8192, false).unwrap();
    assert_eq!((first.counter, second.counter), (1, 2));
    assert_eq!((first.allocations, second.allocations), (2, 4));
    assert_eq!(scalar(&mut store, &instance, "postcalls").unwrap(), 2);
    for _ in 0..2 {
        assert_eq!(
            runtime
                .fresh("fresh", "nav", caps, Checkpoint::None)
                .unwrap()
                .counter,
            1
        );
    }
}
