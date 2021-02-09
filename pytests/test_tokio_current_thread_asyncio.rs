mod common;
mod tokio_asyncio;

#[pyo3_asyncio::tokio::main(flavor = "current_thread")]
async fn main() -> pyo3::PyResult<()> {
    pyo3_asyncio::testing::main().await
}
