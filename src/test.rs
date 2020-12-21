use std::{future::Future, pin::Pin};

use futures::stream::{Stream, StreamExt};
use pyo3::prelude::*;

pub struct Test {
    pub name: String,
    pub task: Pin<Box<dyn Future<Output = PyResult<()>> + Send>>,
}

impl Test {
    pub fn new_async(
        name: String,
        fut: impl Future<Output = PyResult<()>> + Send + 'static,
    ) -> Self {
        Self {
            name,
            task: Box::pin(fut),
        }
    }

    pub fn new_sync<F>(name: String, func: F) -> Self
    where
        F: FnOnce() -> PyResult<()> + Send + 'static,
    {
        Self {
            name,
            task: Box::pin(async move { crate::spawn_blocking(func).await.unwrap() }),
        }
    }
}

pub async fn test_harness(tests: impl Stream<Item = Test>) -> PyResult<()> {
    tests
        .for_each_concurrent(Some(4), |test| async move {
            test.task.await.unwrap();
            println!("test {} ... ok", test.name);
        })
        .await;

    Ok(())
}
