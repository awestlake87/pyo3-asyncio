mod common;
mod tokio_asyncio;

// TODO: Fix current thread init
pyo3_asyncio::tokio::test_main!(
    #[pyo3_asyncio::tokio::main(flavor = "current_thread")],
    "PyO3 Asyncio Tokio Current Thread Test Suite"
);
