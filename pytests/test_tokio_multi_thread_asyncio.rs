mod common;
mod tokio_asyncio;

fn main() {
    pyo3_asyncio::tokio::init_multi_thread();

    tokio_asyncio::test_main("PyO3 Asyncio Tokio Multi-Thread Test Suite");
}
