mod common;
mod tokio_asyncio;

#[pyo3_asyncio::tokio::main]
async fn main() -> pyo3::PyResult<()> {
    pyo3_asyncio::testing::main().await
}
