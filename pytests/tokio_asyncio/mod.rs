use std::{rc::Rc, time::Duration};

use pyo3::{prelude::*, wrap_pyfunction};

use crate::common;

#[pyfunction]
fn sleep_for(py: Python, secs: &PyAny) -> PyResult<PyObject> {
    let secs = secs.extract()?;

    pyo3_asyncio::tokio::into_coroutine(py, async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        Python::with_gil(|py| Ok(py.None()))
    })
}

#[pyo3_asyncio::tokio::test]
async fn test_into_coroutine() -> PyResult<()> {
    let fut = Python::with_gil(|py| {
        let sleeper_mod = PyModule::new(py, "rust_sleeper")?;

        sleeper_mod.add_wrapped(wrap_pyfunction!(sleep_for))?;

        let test_mod = PyModule::from_code(
            py,
            common::TEST_MOD,
            "test_rust_coroutine/test_mod.py",
            "test_mod",
        )?;

        pyo3_asyncio::into_future(
            test_mod.call_method1("sleep_for_1s", (sleeper_mod.getattr("sleep_for")?,))?,
        )
    })?;

    fut.await?;

    Ok(())
}

#[pyo3_asyncio::tokio::test]
async fn test_async_sleep() -> PyResult<()> {
    let asyncio =
        Python::with_gil(|py| py.import("asyncio").map(|asyncio| PyObject::from(asyncio)))?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    Python::with_gil(|py| {
        pyo3_asyncio::into_future(asyncio.as_ref(py).call_method1("sleep", (1.0,))?)
    })?
    .await?;

    Ok(())
}

#[pyo3_asyncio::tokio::test]
fn test_blocking_sleep() -> PyResult<()> {
    common::test_blocking_sleep()
}

#[pyo3_asyncio::tokio::test]
async fn test_into_future() -> PyResult<()> {
    common::test_into_future().await
}

#[pyo3_asyncio::tokio::test]
async fn test_other_awaitables() -> PyResult<()> {
    common::test_other_awaitables().await
}

#[pyo3_asyncio::tokio::test]
fn test_init_twice() -> PyResult<()> {
    common::test_init_twice()
}

#[pyo3_asyncio::tokio::test]
fn test_init_tokio_twice() -> PyResult<()> {
    // tokio has already been initialized in test main. call these functions to
    // make sure they don't cause problems with the other tests.
    pyo3_asyncio::tokio::init_multi_thread_once();
    pyo3_asyncio::tokio::init_current_thread_once();

    Ok(())
}

#[pyo3_asyncio::tokio::test]
fn test_local_set_coroutine() -> PyResult<()> {
    tokio::task::LocalSet::new().block_on(pyo3_asyncio::tokio::get_runtime(), async {
        Python::with_gil(|py| {
            let non_send_secs = Rc::new(1);

            let py_future = pyo3_asyncio::tokio::into_local_py_future(py, async move {
                tokio::time::sleep(Duration::from_secs(*non_send_secs)).await;
                Ok(Python::with_gil(|py| py.None()))
            })?;

            pyo3_asyncio::into_future(py_future.as_ref(py))
        })?
        .await?;

        Ok(())
    })
}
