mod common;

use std::{rc::Rc, time::Duration};

use async_std::task;
use pyo3::{prelude::*, wrap_pyfunction};

#[pyfunction]
fn sleep_for(py: Python, secs: &PyAny) -> PyResult<PyObject> {
    let secs = secs.extract()?;

    pyo3_asyncio::async_std::into_coroutine(py, async move {
        task::sleep(Duration::from_secs(secs)).await;
        Python::with_gil(|py| Ok(py.None()))
    })
}

#[pyo3_asyncio::async_std::test]
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

        pyo3_asyncio::async_std::into_future(
            test_mod.call_method1("sleep_for_1s", (sleeper_mod.getattr("sleep_for")?,))?,
        )
    })?;

    fut.await?;

    Ok(())
}

#[pyo3_asyncio::async_std::test]
async fn test_async_sleep() -> PyResult<()> {
    let asyncio =
        Python::with_gil(|py| py.import("asyncio").map(|asyncio| PyObject::from(asyncio)))?;

    task::sleep(Duration::from_secs(1)).await;

    Python::with_gil(|py| {
        pyo3_asyncio::async_std::into_future(asyncio.as_ref(py).call_method1("sleep", (1.0,))?)
    })?
    .await?;

    Ok(())
}

#[pyo3_asyncio::async_std::test]
fn test_blocking_sleep(_event_loop: PyObject) -> PyResult<()> {
    common::test_blocking_sleep()
}

#[pyo3_asyncio::async_std::test]
async fn test_into_future() -> PyResult<()> {
    common::test_into_future(Python::with_gil(|py| {
        pyo3_asyncio::async_std::task_event_loop(py).unwrap().into()
    }))
    .await
}

#[pyo3_asyncio::async_std::test]
async fn test_other_awaitables() -> PyResult<()> {
    common::test_other_awaitables(Python::with_gil(|py| {
        pyo3_asyncio::async_std::task_event_loop(py).unwrap().into()
    }))
    .await
}

#[pyo3_asyncio::async_std::test]
fn test_init_twice(_event_loop: PyObject) -> PyResult<()> {
    common::test_init_twice()
}

#[pyo3_asyncio::async_std::test]
async fn test_panic() -> PyResult<()> {
    let fut = Python::with_gil(|py| -> PyResult<_> {
        pyo3_asyncio::async_std::into_future(
            pyo3_asyncio::async_std::into_coroutine(py, async {
                panic!("this panic was intentional!")
            })?
            .as_ref(py),
        )
    })?;

    match fut.await {
        Ok(_) => panic!("coroutine should panic"),
        Err(e) => Python::with_gil(|py| {
            if e.is_instance::<pyo3_asyncio::err::RustPanic>(py) {
                Ok(())
            } else {
                panic!("expected RustPanic err")
            }
        }),
    }
}

#[pyo3_asyncio::async_std::main]
async fn main() -> pyo3::PyResult<()> {
    pyo3_asyncio::testing::main().await
}

#[pyo3_asyncio::async_std::test]
async fn test_local_coroutine() -> PyResult<()> {
    Python::with_gil(|py| {
        let non_send_secs = Rc::new(1);

        let py_future = pyo3_asyncio::async_std::local_future_into_py(py, async move {
            async_std::task::sleep(Duration::from_secs(*non_send_secs)).await;
            Ok(Python::with_gil(|py| py.None()))
        })?;

        pyo3_asyncio::async_std::into_future(py_future)
    })?
    .await?;

    Ok(())
}
