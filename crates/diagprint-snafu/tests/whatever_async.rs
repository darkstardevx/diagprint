#![cfg(feature = "futures")]

use diagprint::Reporter;
use diagprint_snafu::{
    DiagprintTryFutureExt, DiagprintTryStreamExt, SnafuIdentity, WhateverDiagnosticContext,
};
use futures::{StreamExt, executor::block_on, future, stream};
use std::io;

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-snafu-whatever-async-test")
        .color(false)
        .build()
        .unwrap()
}

fn context(message: &str) -> WhateverDiagnosticContext {
    WhateverDiagnosticContext::new(SnafuIdentity::new("async.operation").unwrap(), message)
}

#[test]
fn future_strong_whatever_is_lazy_and_captures_on_error() {
    let reporter = reporter();

    let wrapped = future::ready::<Result<u8, io::Error>>(Err(io::Error::other("boom")))
        .diagprint_whatever_context(&reporter, context("future operation failed"));

    let captured = block_on(wrapped).unwrap_err();

    assert_eq!(captured.error().to_string(), "future operation failed");
    assert_eq!(captured.output().unwrap().whatever_nodes(), 1);
}

#[test]
fn stream_strong_whatever_captures_each_error_independently() {
    let reporter = reporter();

    let source = stream::iter(vec![
        Err::<u8, io::Error>(io::Error::other("one")),
        Ok(7),
        Err::<u8, io::Error>(io::Error::other("two")),
    ]);

    let items = block_on(
        source
            .diagprint_whatever_context(&reporter, context("stream operation failed"))
            .collect::<Vec<_>>(),
    );

    assert_eq!(items.len(), 3);
    assert!(items[0].is_err());
    assert_eq!(items[1].as_ref().unwrap(), &7);
    assert!(items[2].is_err());

    for captured in [
        items[0].as_ref().err().unwrap(),
        items[2].as_ref().err().unwrap(),
    ] {
        assert_eq!(captured.output().unwrap().whatever_nodes(), 1);
    }

    assert!(!std::ptr::eq(
        items[0].as_ref().err().unwrap().report().unwrap(),
        items[2].as_ref().err().unwrap().report().unwrap(),
    ));
}
