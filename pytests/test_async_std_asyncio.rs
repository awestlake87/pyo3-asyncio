mod common;

use std::{rc::Rc, time::Duration};

use async_std::task;
use pyo3::{
    prelude::*,
    proc_macro::pymodule,
    types::{IntoPyDict, PyType},
    wrap_pyfunction, wrap_pymodule,
};

#[pyfunction]
#[allow(deprecated)]
fn sleep_into_coroutine(py: Python, secs: &PyAny) -> PyResult<PyObject> {
    let secs = secs.extract()?;

    pyo3_asyncio::async_std::into_coroutine(py, async move {
        task::sleep(Duration::from_secs(secs)).await;
        Python::with_gil(|py| Ok(py.None()))
    })
}

#[pyfunction]
fn sleep<'p>(py: Python<'p>, secs: &'p PyAny) -> PyResult<&'p PyAny> {
    let secs = secs.extract()?;

    pyo3_asyncio::async_std::future_into_py(py, async move {
        task::sleep(Duration::from_secs(secs)).await;
        Python::with_gil(|py| Ok(py.None()))
    })
}

#[pyo3_asyncio::async_std::test]
fn test_into_coroutine() -> PyResult<()> {
    #[allow(deprecated)]
    Python::with_gil(|py| {
        let sleeper_mod = PyModule::new(py, "rust_sleeper")?;

        sleeper_mod.add_wrapped(wrap_pyfunction!(sleep_into_coroutine))?;

        let test_mod = PyModule::from_code(
            py,
            common::TEST_MOD,
            "test_into_coroutine_mod.py",
            "test_into_coroutine_mod",
        )?;

        let fut = pyo3_asyncio::into_future(test_mod.call_method1(
            "sleep_for_1s",
            (sleeper_mod.getattr("sleep_into_coroutine")?,),
        )?)?;

        pyo3_asyncio::async_std::run_until_complete(
            pyo3_asyncio::get_event_loop(py),
            async move {
                fut.await?;
                Ok(())
            },
        )?;

        Ok(())
    })
}

#[pyo3_asyncio::async_std::test]
async fn test_future_into_py() -> PyResult<()> {
    let fut = Python::with_gil(|py| {
        let sleeper_mod = PyModule::new(py, "rust_sleeper")?;

        sleeper_mod.add_wrapped(wrap_pyfunction!(sleep))?;

        let test_mod = PyModule::from_code(
            py,
            common::TEST_MOD,
            "test_future_into_py_mod.py",
            "test_future_into_py_mod",
        )?;

        pyo3_asyncio::async_std::into_future(
            test_mod.call_method1("sleep_for_1s", (sleeper_mod.getattr("sleep")?,))?,
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
fn test_blocking_sleep() -> PyResult<()> {
    common::test_blocking_sleep()
}

#[pyo3_asyncio::async_std::test]
async fn test_into_future() -> PyResult<()> {
    common::test_into_future(Python::with_gil(|py| {
        pyo3_asyncio::async_std::get_current_loop(py)
            .unwrap()
            .into()
    }))
    .await
}

#[pyo3_asyncio::async_std::test]
async fn test_into_future_0_13() -> PyResult<()> {
    common::test_into_future_0_13().await
}

#[pyo3_asyncio::async_std::test]
async fn test_other_awaitables() -> PyResult<()> {
    common::test_other_awaitables(Python::with_gil(|py| {
        pyo3_asyncio::async_std::get_current_loop(py)
            .unwrap()
            .into()
    }))
    .await
}

#[pyo3_asyncio::async_std::test]
async fn test_panic() -> PyResult<()> {
    let fut = Python::with_gil(|py| -> PyResult<_> {
        pyo3_asyncio::async_std::into_future(pyo3_asyncio::async_std::future_into_py(py, async {
            panic!("this panic was intentional!")
        })?)
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

#[pyo3_asyncio::async_std::test]
async fn test_local_future_into_py() -> PyResult<()> {
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

#[pyo3_asyncio::async_std::test]
async fn test_cancel() -> PyResult<()> {
    let py_future = Python::with_gil(|py| -> PyResult<PyObject> {
        Ok(pyo3_asyncio::async_std::future_into_py(py, async {
            async_std::task::sleep(Duration::from_secs(1)).await;
            Ok(Python::with_gil(|py| py.None()))
        })?
        .into())
    })?;

    if let Err(e) = Python::with_gil(|py| -> PyResult<_> {
        py_future.as_ref(py).call_method0("cancel")?;
        pyo3_asyncio::async_std::into_future(py_future.as_ref(py))
    })?
    .await
    {
        Python::with_gil(|py| -> PyResult<()> {
            assert!(py
                .import("asyncio")?
                .getattr("CancelledError")?
                .downcast::<PyType>()
                .unwrap()
                .is_instance(e.pvalue(py))?);
            Ok(())
        })?;
    } else {
        panic!("expected CancelledError");
    }

    Ok(())
}

/// This module is implemented in Rust.
#[pymodule]
fn test_mod(_py: Python, m: &PyModule) -> PyResult<()> {
    #![allow(deprecated)]
    #[pyfn(m, "sleep")]
    fn sleep(py: Python) -> PyResult<&PyAny> {
        pyo3_asyncio::async_std::future_into_py(py, async move {
            async_std::task::sleep(Duration::from_millis(500)).await;
            Ok(Python::with_gil(|py| py.None()))
        })
    }

    Ok(())
}

const TEST_CODE: &str = r#"
async def main():
    return await test_mod.sleep()

asyncio.new_event_loop().run_until_complete(main())
"#;

#[pyo3_asyncio::async_std::test]
fn test_multiple_asyncio_run() -> PyResult<()> {
    Python::with_gil(|py| {
        pyo3_asyncio::async_std::run(py, async move {
            async_std::task::sleep(Duration::from_millis(500)).await;
            Ok(())
        })?;
        pyo3_asyncio::async_std::run(py, async move {
            async_std::task::sleep(Duration::from_millis(500)).await;
            Ok(())
        })?;

        let d = [
            ("asyncio", py.import("asyncio")?.into()),
            ("test_mod", wrap_pymodule!(test_mod)(py)),
        ]
        .into_py_dict(py);

        py.run(TEST_CODE, Some(d), None)?;
        py.run(TEST_CODE, Some(d), None)?;
        Ok(())
    })
}

#[allow(deprecated)]
fn main() -> pyo3::PyResult<()> {
    pyo3::prepare_freethreaded_python();

    Python::with_gil(|py| {
        // into_coroutine requires the 0.13 API
        pyo3_asyncio::try_init(py)?;
        pyo3_asyncio::async_std::run(py, pyo3_asyncio::testing::main())
    })
}
