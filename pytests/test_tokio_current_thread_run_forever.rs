mod tokio_run_forever;

fn main() {
    let mut builder = tokio::runtime::Builder::new_current_thread();
    builder.enable_all();

    pyo3_asyncio::tokio::init(builder);
    std::thread::spawn(move || {
        pyo3_asyncio::tokio::get_runtime().block_on(std::future::pending::<()>());
    });

    tokio_run_forever::test_main();
}
