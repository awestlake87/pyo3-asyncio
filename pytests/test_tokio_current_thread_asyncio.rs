mod common;
mod tokio_asyncio;

pyo3_asyncio::testing::test_main!(
    #[pyo3_asyncio::tokio::main(flavor = "current_thread")],
    "PyO3 Asyncio Tokio Current Thread Test Suite"
);
