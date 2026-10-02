//! Parallel reading of the DB files whose keys are compared.
//!
//! Reading is I/O bound: on a real library (228 Workshop mods, 18.5k DB files, 125 MB) one
//! thread spent ~3 s of a ~4.3 s pass in reads, and decoding keys took ~0.3 s. Packs are therefore
//! read by a few threads, one pack at a time each, and results stream back through a bounded
//! channel so the decoded keys of the whole library are never held at once.
use crate::{
    Error, Result, db,
    localization::Message,
    pack::{self, PackedFile},
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

/// Upper bound for reader threads; more only adds seeking on a spinning disk.
const MAX_THREADS: usize = 8;
const QUEUE: usize = 64;

/// One DB file to read: its position in the report and its index entry.
pub(super) struct File<'a> {
    pub(super) table: usize,
    pub(super) file: usize,
    pub(super) table_name: &'a str,
    pub(super) entry: Option<&'a PackedFile>,
}

/// All files of one pack, read with a single handle.
pub(super) struct Job<'a> {
    pub(super) owner: usize,
    pub(super) path: &'a Path,
    pub(super) files: Vec<File<'a>>,
}

pub(super) enum Outcome {
    Keys {
        table: usize,
        file: usize,
        keys: Vec<Vec<String>>,
    },
    Failed {
        table: usize,
        file: usize,
        owner: usize,
        reason: Message,
    },
}

fn read_job(job: &Job, cancelled: &AtomicBool, send: &mut dyn FnMut(Outcome) -> bool) {
    let mut reader = pack::Reader::open(job.path);
    for file in &job.files {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }
        let keys = match (&mut reader, file.entry) {
            (Ok(reader), Some(entry)) => reader
                .read(entry)
                .and_then(|bytes| db::read_keys(&bytes, file.table_name))
                .map_err(|e| e.message()),
            (Err(e), _) => Err(e.message()),
            (Ok(_), None) => Err(crate::message!(
                "the file is missing from the pack index",
                "файла нет в индексе pack"
            )),
        };
        let outcome = match keys {
            Ok(keys) => Outcome::Keys {
                table: file.table,
                file: file.file,
                keys,
            },
            Err(reason) => Outcome::Failed {
                table: file.table,
                file: file.file,
                owner: job.owner,
                reason,
            },
        };
        if !send(outcome) {
            return;
        }
    }
}

/// Reads every job and hands each outcome to `merge` on the calling thread.
pub(super) fn read_all(
    jobs: &[Job],
    cancelled: &AtomicBool,
    mut merge: impl FnMut(Outcome),
) -> Result<()> {
    let threads = thread::available_parallelism()
        .map_or(1, |count| count.get())
        .clamp(1, MAX_THREADS)
        .min(jobs.len());
    if threads <= 1 {
        for job in jobs {
            read_job(job, cancelled, &mut |outcome| {
                merge(outcome);
                true
            });
        }
    } else {
        let next = AtomicUsize::new(0);
        let (sender, receiver) = mpsc::sync_channel(QUEUE);
        thread::scope(|scope| {
            for _ in 0..threads {
                let sender = sender.clone();
                let next = &next;
                scope.spawn(move || {
                    while let Some(job) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                        // A closed channel means the receiver is gone; stop quietly.
                        read_job(job, cancelled, &mut |outcome| sender.send(outcome).is_ok());
                        if cancelled.load(Ordering::Relaxed) {
                            return;
                        }
                    }
                });
            }
            drop(sender);
            for outcome in receiver {
                merge(outcome);
            }
        });
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    Ok(())
}
