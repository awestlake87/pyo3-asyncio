mod common;
mod tokio_asyncio;

pyo3_asyncio::testing::test_main!(
    #[pyo3_asyncio::tokio::main],
    "PyO3 Asyncio Tokio Multi Thread Test Suite"
);
