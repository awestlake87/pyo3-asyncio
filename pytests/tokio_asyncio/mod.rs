use std::{convert::TryFrom, future::Future, time::Duration};

use pyo3::{prelude::*, wrap_pyfunction};

use pyo3_asyncio::{testing::Test, tokio::testing::new_sync_test};

// enforce the inclusion of the common module
use crate::common;

#[pyfunction]
fn sleep_for(py: Python, secs: &PyAny) -> PyResult<PyObject> {
    let secs = secs.extract()?;

    pyo3_asyncio::tokio::into_coroutine(py, async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        Python::with_gil(|py| Ok(py.None()))
    })
}

fn test_into_coroutine(
    py: Python,
) -> PyResult<impl Future<Output = PyResult<()>> + Send + 'static> {
    let sleeper_mod: Py<PyModule> = PyModule::new(py, "rust_sleeper")?.into();

    sleeper_mod
        .as_ref(py)
        .add_wrapped(wrap_pyfunction!(sleep_for))?;

    let test_mod: PyObject = PyModule::from_code(
        py,
        common::TEST_MOD,
        "test_rust_coroutine/test_mod.py",
        "test_mod",
    )?
    .into();

    Ok(async move {
        Python::with_gil(|py| {
            pyo3_asyncio::PyFuture::try_from(
                test_mod
                    .call_method1(py, "sleep_for_1s", (sleeper_mod.getattr(py, "sleep_for")?,))?
                    .as_ref(py),
            )
        })?
        .await?;
        Ok(())
    })
}

fn test_async_sleep<'p>(
    py: Python<'p>,
) -> PyResult<impl Future<Output = PyResult<()>> + Send + 'static> {
    let asyncio = PyObject::from(py.import("asyncio")?);

    Ok(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;

        Python::with_gil(|py| {
            pyo3_asyncio::PyFuture::try_from(asyncio.as_ref(py).call_method1("sleep", (1.0,))?)
        })?
        .await?;

        Ok(())
    })
}

pub(super) fn test_main(suite_name: &str) {
    pyo3_asyncio::tokio::testing::test_main(
        suite_name,
        vec![
            Test::new_async(
                "test_async_sleep".into(),
                Python::with_gil(|py| {
                    test_async_sleep(py)
                        .map_err(|e| {
                            e.print_and_set_sys_last_vars(py);
                        })
                        .unwrap()
                }),
            ),
            new_sync_test("test_blocking_sleep".into(), || {
                common::test_blocking_sleep();
                Ok(())
            }),
            Test::new_async(
                "test_into_coroutine".into(),
                Python::with_gil(|py| {
                    test_into_coroutine(py)
                        .map_err(|e| {
                            e.print_and_set_sys_last_vars(py);
                        })
                        .unwrap()
                }),
            ),
            Test::new_async(
                "test_py_future".into(),
                Python::with_gil(|py| {
                    common::test_py_future(py)
                        .map_err(|e| {
                            e.print_and_set_sys_last_vars(py);
                        })
                        .unwrap()
                }),
            ),
        ],
    )
}
