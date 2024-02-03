mod common;
mod wyfo_asyncio;

use pyo3::prelude::*;

fn main() -> pyo3::PyResult<()> {
    pyo3::prepare_freethreaded_python();

    let args = pyo3_asyncio::testing::parse_args();

    Python::with_gil(|py| {
        pyo3_asyncio::wyfo::run(py, async move {
            pyo3_asyncio::testing::main_with_args(pyo3_asyncio::testing::Args {
                concurrent: false,
                ..args
            })
            .await
        })
    })
}
