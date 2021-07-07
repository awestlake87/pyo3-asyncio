mod common;
mod tokio_asyncio;

use pyo3::prelude::*;

#[allow(deprecated)]
fn main() -> pyo3::PyResult<()> {
    Python::with_gil(|py| {
        // into_coroutine requires the 0.13 API
        pyo3_asyncio::try_init(py)?;
        pyo3_asyncio::tokio::init_multi_thread();
        pyo3_asyncio::tokio::run(py, pyo3_asyncio::testing::main())
    })
}
