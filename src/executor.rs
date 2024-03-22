use smol::{lock::Semaphore, LocalExecutor};

pub struct DigExecutor<'a> {
    // _executor: Rc<RefCell<LocalExecutor<'a>>>,
    // _limiter: Rc<RefCell<Semaphore>>,
    pub executor: LocalExecutor<'a>,
    pub limiter: Semaphore,
}

impl<'a> DigExecutor<'a> {
    pub fn new(concurrency: usize) -> Self {
        DigExecutor {
            // _executor: Rc::new(RefCell::new(LocalExecutor::new())),
            // _limiter: Rc::new(RefCell::new(Semaphore::new(concurrency))),
            executor: LocalExecutor::new(),
            limiter: Semaphore::new(concurrency),
        }
    }
}
