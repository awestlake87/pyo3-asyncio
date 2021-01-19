use std::{future::Future, thread, time::Duration};

use async_std::task;
use pyo3::{prelude::*, wrap_pyfunction};

use pyo3_asyncio::{
    async_std::testing::{new_sync_test, test_main},
    testing::Test,
};

#[pyfunction]
fn sleep_for(py: Python, secs: &PyAny) -> PyResult<PyObject> {
    let secs = secs.extract()?;

    pyo3_asyncio::async_std::into_coroutine(py, async move {
        task::sleep(Duration::from_secs(secs)).await;
        Python::with_gil(|py| Ok(py.None()))
    })
}

const TEST_MOD: &'static str = r#"
import asyncio 

async def py_sleep(duration):
    await asyncio.sleep(duration)

async def sleep_for_1s(sleep_for):
    await sleep_for(1)
"#;

fn test_into_coroutine(
    py: Python,
) -> PyResult<impl Future<Output = PyResult<()>> + Send + 'static> {
    let sleeper_mod: Py<PyModule> = PyModule::new(py, "rust_sleeper")?.into();

    sleeper_mod
        .as_ref(py)
        .add_wrapped(wrap_pyfunction!(sleep_for))?;

    let test_mod: PyObject =
        PyModule::from_code(py, TEST_MOD, "test_rust_coroutine/test_mod.py", "test_mod")?.into();

    Ok(async move {
        Python::with_gil(|py| {
            pyo3_asyncio::into_future(
                py,
                test_mod
                    .call_method1(py, "sleep_for_1s", (sleeper_mod.getattr(py, "sleep_for")?,))?
                    .as_ref(py),
            )
        })?
        .await?;
        Ok(())
    })
}

fn test_into_future(py: Python) -> PyResult<impl Future<Output = PyResult<()>> + Send + 'static> {
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

fn test_async_sleep<'p>(
    py: Python<'p>,
) -> PyResult<impl Future<Output = PyResult<()>> + Send + 'static> {
    let asyncio = PyObject::from(py.import("asyncio")?);

    Ok(async move {
        task::sleep(Duration::from_secs(1)).await;

        Python::with_gil(|py| {
            pyo3_asyncio::into_future(py, asyncio.as_ref(py).call_method1("sleep", (1.0,))?)
        })?
        .await?;

        Ok(())
    })
}

fn test_blocking_sleep() {
    thread::sleep(Duration::from_secs(1));
}

fn main() {
    test_main(
        "PyO3 Asyncio Test Suite",
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
                test_blocking_sleep();
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
                "test_into_future".into(),
                Python::with_gil(|py| {
                    test_into_future(py)
                        .map_err(|e| {
                            e.print_and_set_sys_last_vars(py);
                        })
                        .unwrap()
                }),
            ),
        ],
    )
}
