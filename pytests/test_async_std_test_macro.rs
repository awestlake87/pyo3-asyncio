use std::{thread, time::Duration};

use pyo3::prelude::*;

#[pyo3_asyncio::async_std::test]
fn test() {
    thread::sleep(Duration::from_secs(1));
}

#[pyo3_asyncio::async_std::test]
async fn test_sleep() -> PyResult<()> {
    async_std::task::sleep(Duration::from_secs(1)).await;

    Ok(())
}

pyo3_asyncio::async_std::test_main!("PyO3 test async-std test macros");
