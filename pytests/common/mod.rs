use std::{thread, time::Duration};

use pyo3::prelude::*;

pub(super) const TEST_MOD: &'static str = r#"
import asyncio 

async def py_sleep(duration):
    await asyncio.sleep(duration)

async def sleep_for_1s(sleep_for):
    await sleep_for(1)
"#;

pub(super) async fn test_into_future() -> PyResult<()> {
    let fut = Python::with_gil(|py| {
        let test_mod =
            PyModule::from_code(py, TEST_MOD, "test_rust_coroutine/test_mod.py", "test_mod")?;

        pyo3_asyncio::into_future(test_mod.call_method1("py_sleep", (1.into_py(py),))?)
    })?;

    fut.await?;

    Ok(())
}

pub(super) fn test_blocking_sleep() -> PyResult<()> {
    thread::sleep(Duration::from_secs(1));
    Ok(())
}
