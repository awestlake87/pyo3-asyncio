use std::{future::Future, thread, time::Duration};

use futures::stream::{self};
use pyo3::prelude::*;

use pyo3_asyncio::testing::{test_main, Test};

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

fn main() {
    test_main(stream::iter(vec![
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
        Test::new_sync("test_blocking_sleep".into(), || {
            test_blocking_sleep();
            Ok(())
        }),
    ]))
}
