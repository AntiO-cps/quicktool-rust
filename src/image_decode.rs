//! A bounded latest-request worker: typing keeps at most one queued image input.
use std::sync::{Arc, Condvar, Mutex, mpsc};
struct Pending {
    next: Option<(u64, Arc<str>)>,
    closed: bool,
}
pub struct LatestWorker<T> {
    pending: Arc<(Mutex<Pending>, Condvar)>,
    pub results: mpsc::Receiver<(u64, Result<T, String>)>,
    serial: u64,
}
impl<T: Send + 'static> LatestWorker<T> {
    pub fn new(
        decode: impl Fn(&str) -> Result<T, String> + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let pending = Arc::new((
            Mutex::new(Pending {
                next: None,
                closed: false,
            }),
            Condvar::new(),
        ));
        let worker = pending.clone();
        let (tx, results) = mpsc::channel();
        std::thread::spawn(move || {
            loop {
                let job = {
                    let (lock, changed) = &*worker;
                    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
                    while state.next.is_none() && !state.closed {
                        state = changed.wait(state).unwrap_or_else(|e| e.into_inner());
                    }
                    if state.closed {
                        return;
                    }
                    state.next.take().unwrap()
                };
                let result = decode(&job.1);
                if tx.send((job.0, result)).is_err() {
                    return;
                }
                wake();
            }
        });
        Self {
            pending,
            results,
            serial: 0,
        }
    }
    pub fn request(&mut self, text: Arc<str>) -> u64 {
        self.serial = self.serial.wrapping_add(1);
        let (lock, changed) = &*self.pending;
        lock.lock().unwrap_or_else(|e| e.into_inner()).next = Some((self.serial, text));
        changed.notify_one();
        self.serial
    }
}
impl<T> Drop for LatestWorker<T> {
    fn drop(&mut self) {
        let (lock, changed) = &*self.pending;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        state.next = None;
        changed.notify_one();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_inputs_replace_older_pending_work() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut worker = LatestWorker::new(
            move |text| {
                started_tx.send(text.to_string()).unwrap();
                if text == "first" {
                    release_rx.recv().unwrap();
                }
                Ok(text.to_string())
            },
            || {},
        );
        let a = worker.request(Arc::from("first"));
        assert_eq!(started_rx.recv().unwrap(), "first");
        worker.request(Arc::from("obsolete"));
        let c = worker.request(Arc::from("current"));
        release_tx.send(()).unwrap();
        assert_eq!(worker.results.recv().unwrap(), (a, Ok("first".into())));
        assert_eq!(started_rx.recv().unwrap(), "current");
        assert_eq!(worker.results.recv().unwrap(), (c, Ok("current".into())));
        assert!(started_rx.try_recv().is_err());
    }
}
