use std::{future::Future, thread, time::Duration};

use pyo3::prelude::*;

pub(super) const TEST_MOD: &'static str = r#"
import asyncio 

async def py_sleep(duration):
    await asyncio.sleep(duration)

async def sleep_for_1s(sleep_for):
    await sleep_for(1)
"#;

pub(super) fn test_into_future(
    py: Python,
) -> PyResult<impl Future<Output = PyResult<()>> + Send + 'static> {
    let test_mod: PyObject =
        PyModule::from_code(py, TEST_MOD, "test_rust_coroutine/test_mod.py", "test_mod")?.into();

    Ok(async move {
        Python::with_gil(|py| {
            pyo3_asyncio::into_future(
                py,
                test_mod
                    .call_method1(py, "py_sleep", (1.into_py(py),))?
                    .as_ref(py),
            )
        })?
        .await?;
        Ok(())
    })
}

pub(super) fn test_blocking_sleep() {
    thread::sleep(Duration::from_secs(1));
}
