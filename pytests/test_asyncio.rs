use std::{future::Future, thread, time::Duration};

use futures::stream::{self};
use pyo3::prelude::*;

use pyo3_asyncio::test::{parse_args, test_harness, Test};

fn dump_err(py: Python<'_>) -> impl FnOnce(PyErr) + '_ {
    move |e| {
        // We can't display Python exceptions via std::fmt::Display,
        // so print the error here manually.
        e.print_and_set_sys_last_vars(py);
    }
}

fn test_async_sleep<'p>(
    py: Python<'p>,
) -> PyResult<impl Future<Output = PyResult<()>> + Send + 'static> {
    let asyncio = PyObject::from(py.import("asyncio")?);

    Ok(async move {
        println!("async sleep for 1s");
        tokio::time::sleep(Duration::from_secs(1)).await;

        println!("asyncio sleep for 1s");
        Python::with_gil(|py| {
            pyo3_asyncio::into_future(py, asyncio.as_ref(py).call_method1("sleep", (1.0,))?)
        })?
        .await?;

        println!("success!");

        Ok(())
    })
}

fn test_blocking_sleep() {
    println!("blocking sleep for 1s");
    thread::sleep(Duration::from_secs(1));
    println!("success");
}

fn py_main(py: Python) -> PyResult<()> {
    let args = parse_args("Pyo3 Asyncio Test Suite");

    pyo3_asyncio::try_init(py)?;

    pyo3_asyncio::run_until_complete(
        py,
        test_harness(
            stream::iter(vec![
                Test::new_async("test_async_sleep".into(), test_async_sleep(py)?),
                Test::new_sync("test_blocking_sleep".into(), || {
                    test_blocking_sleep();
                    Ok(())
                }),
            ]),
            args,
        ),
    )?;

    Ok(())
}

fn main() {
    Python::with_gil(|py| {
        py_main(py).map_err(dump_err(py)).unwrap();
    });
}
