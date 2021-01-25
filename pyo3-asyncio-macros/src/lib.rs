#![forbid(unsafe_code, future_incompatible, rust_2018_idioms)]
#![deny(missing_debug_implementations, nonstandard_style)]
#![recursion_limit = "512"]

mod tokio;

use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

/// Enables an async main function that uses the async-std runtime.
///
/// # Examples
///
/// ```ignore
/// #[pyo3_asyncio::async_std::main]
/// async fn main() -> PyResult<()> {
///     Ok(())
/// }
/// ```
#[cfg(not(test))] // NOTE: exporting main breaks tests, we should file an issue.
#[proc_macro_attribute]
pub fn async_std_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);

    let ret = &input.sig.output;
    let inputs = &input.sig.inputs;
    let name = &input.sig.ident;
    let body = &input.block;
    let attrs = &input.attrs;
    let vis = &input.vis;

    if name != "main" {
        return TokenStream::from(quote_spanned! { name.span() =>
            compile_error!("only the main function can be tagged with #[async_std::main]"),
        });
    }

    if input.sig.asyncness.is_none() {
        return TokenStream::from(quote_spanned! { input.span() =>
            compile_error!("the async keyword is missing from the function declaration"),
        });
    }

    let result = quote! {
        #vis fn main() {
            #(#attrs)*
            async fn main(#inputs) #ret {
                #body
            }

            pyo3::Python::with_gil(|py| {
                pyo3_asyncio::with_runtime(py, || {
                    pyo3_asyncio::async_std::run_until_complete(py, main())?;

                    Ok(())
                })
                .map_err(|e| {
                    e.print_and_set_sys_last_vars(py);
                })
                .unwrap();
            });
        }
    };

    result.into()
}

/// Enables an async main function that uses the tokio runtime.
///
/// # Arguments
/// * `flavor` - selects the type of tokio runtime ["multi_thread", "current_thread"]
/// * `worker_threads` - number of worker threads, defaults to the number of CPUs on the system
///
/// # Examples
///
/// Default configuration:
/// ```ignore
/// #[pyo3_asyncio::tokio::main]
/// async fn main() -> PyResult<()> {
///     Ok(())
/// }
/// ```
///
/// Current-thread scheduler:
/// ```ignore
/// #[pyo3_asyncio::tokio::main(flavor = "current_thread")]
/// async fn main() -> PyResult<()> {
///     Ok(())
/// }
/// ```
///
/// Multi-thread scheduler with custom worker thread count:
/// ```ignore
/// #[pyo3_asyncio::tokio::main(flavor = "multi_thread", worker_threads = 10)]
/// async fn main() -> PyResult<()> {
///     Ok(())
/// }
/// ```
#[cfg(not(test))] // NOTE: exporting main breaks tests, we should file an issue.
#[proc_macro_attribute]
pub fn tokio_main(args: TokenStream, item: TokenStream) -> TokenStream {
    tokio::main(args, item, true)
}
