use rayon::{ThreadPool, ThreadPoolBuildError, ThreadPoolBuilder};

pub struct JobSystem {
    pool: ThreadPool,
}

impl JobSystem {
    pub fn new(num_threads: Option<usize>) -> Result<Self, ThreadPoolBuildError> {
        let mut builder = ThreadPoolBuilder::new();
        if let Some(count) = num_threads {
            builder = builder.num_threads(count);
        }

        let pool = builder.build()?;
        Ok(Self { pool })
    }

    pub fn spawn<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.pool.spawn(job);
    }

    pub fn scope<'scope, OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce(&rayon::Scope<'scope>) -> R + Send,
        R: Send,
    {
        self.pool.scope(op)
    }
}

impl Default for JobSystem {
    fn default() -> Self {
        let pool = ThreadPoolBuilder::new()
            .build()
            .expect("failed to create default rayon thread pool");
        Self { pool }
    }
}
