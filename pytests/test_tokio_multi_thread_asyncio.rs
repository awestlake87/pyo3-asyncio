mod common;
mod tokio_asyncio;

pyo3_asyncio::tokio::test_main!("PyO3 Asyncio Test Suite for Tokio Multi-Thread Runtime");
