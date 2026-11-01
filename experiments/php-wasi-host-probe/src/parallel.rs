//! Real PHP requests on separate stores sharing one compiled module and clock.
use super::{Run, check_basic, run_observed};
use anyhow::{Context, Result, ensure};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use wasmtime::{Engine, Module, Trap};

pub const PARALLEL_REQUESTS: usize = 4;
const SHORT_NONCE: u64 = 30_001;
const LONG_NONCE: u64 = 30_002;
const SHORT_DEADLINE: Duration = Duration::from_millis(500);
const LONG_DEADLINE: Duration = Duration::from_secs(2);

fn require_interrupt(result: &Run, label: &str) -> Result<()> {
    ensure!(
        result.completed_body().is_err(),
        "{label} timeout became a publishable response"
    );
    let error = result.exit.as_ref().err().context("missing timeout trap")?;
    ensure!(
        error.downcast_ref::<Trap>() == Some(&Trap::Interrupt),
        "wrong {label} timeout trap: {error:#}"
    );
    Ok(())
}

pub fn verify(engine: &Engine, module: &Module) -> Result<()> {
    // All jobs are joined even when an earlier result or fixture assertion fails.
    let batch = thread::scope(|scope| {
        let jobs: Vec<_> = (0..PARALLEL_REQUESTS)
            .map(|index| scope.spawn(move || check_basic(engine, module, 20_000 + index as u64)))
            .collect();
        jobs.into_iter()
            .map(|job| {
                job.join()
                    .map_err(|_| anyhow::anyhow!("parallel PHP request panicked"))
                    .and_then(|result| result)
            })
            .collect::<Vec<_>>()
    });
    for result in batch {
        result.context("parallel request isolation")?;
    }

    let long_finished = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| -> Result<()> {
        let (ready, receiver) = mpsc::channel();
        let short_ready = ready.clone();
        let short = scope.spawn(move || {
            run_observed(
                engine,
                module,
                "epoch",
                SHORT_NONCE,
                u64::MAX,
                SHORT_DEADLINE,
                Some(short_ready),
            )
        });
        let finished = long_finished.clone();
        let long = scope.spawn(move || {
            let started = Instant::now();
            let result = run_observed(
                engine,
                module,
                "epoch",
                LONG_NONCE,
                u64::MAX,
                LONG_DEADLINE,
                Some(ready),
            );
            finished.store(true, Ordering::Release);
            (result, started.elapsed())
        });

        // Readiness means the per-store epoch callback has run while executing
        // the actual PHP Wasm, rather than merely starting a Rust worker thread.
        let readiness = (|| -> Result<()> {
            let mut entered = Vec::new();
            for _ in 0..2 {
                entered.push(
                    receiver
                        .recv_timeout(Duration::from_secs(3))
                        .context("parallel PHP request did not enter its epoch callback")?,
                );
            }
            entered.sort_unstable();
            ensure!(
                entered == [SHORT_NONCE, LONG_NONCE],
                "invalid parallel request readiness"
            );
            Ok(())
        })();

        let short_result = short
            .join()
            .map_err(|_| anyhow::anyhow!("short PHP deadline worker panicked"))
            .and_then(|result| result);
        let while_long_runs = (|| -> Result<()> {
            readiness?;
            let short_result = short_result?;
            require_interrupt(&short_result, "short")?;
            ensure!(
                !long_finished.load(Ordering::Acquire),
                "short request deadline interrupted its longer sibling"
            );
            check_basic(engine, module, 30_003)
                .context("normal request while a sibling is still running")?;
            ensure!(
                !long_finished.load(Ordering::Acquire),
                "normal request or short timeout interrupted its longer sibling"
            );
            Ok(())
        })();

        // Join before propagating any assertion failure, retaining the shared
        // clock until both bounded infinite-loop fixtures have been reclaimed.
        let long_result = long
            .join()
            .map_err(|_| anyhow::anyhow!("long PHP deadline worker panicked"));
        while_long_runs?;
        let (long_result, elapsed) = long_result?;
        require_interrupt(&long_result?, "long")?;
        ensure!(
            elapsed >= Duration::from_millis(1_500),
            "long request ended before its independent deadline"
        );
        Ok(())
    })
}
