mod common;
mod tokio_asyncio;

// TODO: Fix current thread init
pyo3_asyncio::tokio::test_main!("PyO3 Asyncio Test Suite for Tokio Current-Thread Runtime");
