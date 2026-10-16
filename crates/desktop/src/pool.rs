//! Blocking work on a rayon pool that steps aside while the game runs.
//!
//! File and pack I/O, parsing, conflict analysis, searches, Steam worker round trips and
//! self-update run here; GPUI's executors only await the result. While the game runs the
//! pool keeps [`GAME_THREADS`], so the manager does not compete with it for cores, and it
//! grows back once the game closes.
//!
//! Rayon cannot resize a pool, so a switch replaces it. Dropping the old pool lets the work
//! already queued on it finish before its threads exit: a switch never loses or waits for
//! work, and only work submitted afterwards gets the new width.

use futures_channel::oneshot;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
};
use wh3_core::{localization::Message, message};

/// Width while the game runs. Two rather than one, so a long Steam round trip or download
/// does not hold back a save-list refresh.
const GAME_THREADS: usize = 2;

static POOL: Mutex<Pool> = Mutex::new(Pool::new());
/// Jobs submitted and not finished yet, across every pool generation.
static RUNNING: AtomicUsize = AtomicUsize::new(0);

/// A job that ended in a panic; the panic hook has already reported it.
#[derive(Debug)]
pub struct Interrupted;

impl From<Interrupted> for Message {
    fn from(_: Interrupted) -> Self {
        message!(
            "The background task stopped because of an internal error",
            "Фоновая задача прервалась из-за внутренней ошибки"
        )
    }
}

impl From<Interrupted> for wh3_core::Error {
    fn from(interrupted: Interrupted) -> Self {
        Self::Invalid(interrupted.into())
    }
}

/// Shrinks the pool while the game runs and restores it afterwards; cheap when the state
/// did not change.
pub fn set_game_running(running: bool) {
    lock().set_game_running(running);
}

/// Runs `work` on the pool. `None` means it panicked. Dropping the future does not cancel
/// started work: it finishes, and its result is discarded.
pub fn spawn<F, R>(work: F) -> impl Future<Output = Option<R>>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (done, result) = oneshot::channel();
    lock().submit(move || {
        let _ = done.send(work());
    });
    // A panic drops the sender with the job, which ends the wait.
    async move { result.await.ok() }
}

/// [`spawn`] for fallible work: a panic becomes one more error for the caller to report.
pub fn try_spawn<F, T, E>(work: F) -> impl Future<Output = Result<T, E>>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: From<Interrupted> + Send + 'static,
{
    let result = spawn(work);
    async move { result.await.unwrap_or_else(|| Err(Interrupted.into())) }
}

/// Every submitted job has finished. Tests wait for it: GPUI's test dispatcher only knows
/// its own tasks.
pub fn idle() -> bool {
    RUNNING.load(Ordering::SeqCst) == 0
}

fn lock() -> MutexGuard<'static, Pool> {
    // The pool holds no invariant a panicking holder could break.
    POOL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Width with the game closed: half the cores, as in Zed, so the UI thread and GPUI's own
/// background threads keep the rest.
fn width(game_running: bool) -> usize {
    if game_running {
        return GAME_THREADS;
    }
    std::thread::available_parallelism()
        .map_or(1, |cores| cores.get().div_ceil(2))
        .max(GAME_THREADS)
}

struct Pool {
    game_running: bool,
    /// Built on the first job after start or after a switch.
    threads: Option<rayon::ThreadPool>,
}

impl Pool {
    const fn new() -> Self {
        Self {
            game_running: false,
            threads: None,
        }
    }

    fn set_game_running(&mut self, running: bool) {
        if self.game_running != running {
            self.game_running = running;
            self.threads = None;
        }
    }

    fn submit(&mut self, job: impl FnOnce() + Send + 'static) {
        let running = Running::start();
        let job = move || {
            // Declared first, so released last, after the result was sent: whoever sees an
            // idle pool has already been woken.
            let _running = running;
            // Rayon aborts the process on an unwinding job.
            let _ = catch_unwind(AssertUnwindSafe(job));
        };
        let threads = match self.threads.take().map_or_else(|| self.build(), Ok) {
            Ok(threads) => threads,
            Err(error) => {
                // Dropping the job ends its wait as `None`, as a panic would.
                eprintln!("pool: could not start worker threads: {error}");
                return;
            }
        };
        threads.spawn(job);
        self.threads = Some(threads);
    }

    fn build(&self) -> Result<rayon::ThreadPool, rayon::ThreadPoolBuildError> {
        let mode = if self.game_running { "game" } else { "full" };
        rayon::ThreadPoolBuilder::new()
            .num_threads(width(self.game_running))
            // Visible in profilers and crash dumps.
            .thread_name(move |index| format!("pool-{mode}-{index}"))
            .build()
    }
}

/// Counts a job in [`RUNNING`] until it ends, including by panic.
struct Running;

impl Running {
    fn start() -> Self {
        RUNNING.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn returns_the_result_of_the_work() {
        let sum = futures_executor::block_on(spawn(|| (1..=10).sum::<u32>()));
        assert_eq!(sum, Some(55));
    }

    #[test]
    fn a_panic_becomes_an_error() {
        let result: Result<(), Message> =
            futures_executor::block_on(try_spawn(|| panic!("checked by the test")));
        assert!(result.is_err());
    }

    /// Uses its own pool, so the shared one keeps its width for the other tests.
    #[test]
    fn shrinks_while_the_game_runs_and_keeps_queued_work() {
        let mut pool = Pool::new();
        let width_now = |pool: &mut Pool| {
            let (sender, receiver) = mpsc::channel();
            pool.submit(move || {
                let _ = sender.send(rayon::current_num_threads());
            });
            receiver.recv().ok()
        };
        assert_eq!(width_now(&mut pool), Some(width(false)));

        // Work queued before the switch still finishes on the replaced pool.
        let (release, blocked) = mpsc::channel::<()>();
        let (finished, finished_receiver) = mpsc::channel();
        pool.submit(move || {
            let _ = blocked.recv();
            let _ = finished.send(());
        });
        pool.set_game_running(true);
        assert_eq!(width_now(&mut pool), Some(GAME_THREADS));
        let _ = release.send(());
        assert_eq!(finished_receiver.recv().ok(), Some(()));

        pool.set_game_running(false);
        assert_eq!(width_now(&mut pool), Some(width(false)));
    }
}
