use crate::Cause;
use std::{error::Error, io as std_io};

pub(crate) mod io;

pub(crate) fn cause_chain<I, S>(messages: I) -> Option<Cause>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let messages: Vec<String> = messages.into_iter().map(Into::into).collect();

    let mut current = None;

    for message in messages.into_iter().rev() {
        current = Some(match current.take() {
            Some(source) => Cause::new(message).caused_by(source),
            None => Cause::new(message),
        });
    }

    current
}

pub(crate) fn cause_chain_from_sources<E>(error: &E) -> Option<Cause>
where
    E: Error + ?Sized,
{
    let mut messages = Vec::new();
    let mut current = error.source();

    while let Some(source) = current {
        messages.push(source.to_string());
        current = source.source();
    }

    cause_chain(messages)
}

pub(crate) fn find_io_error_in_sources<E>(error: &E) -> Option<&std_io::Error>
where
    E: Error + ?Sized + 'static,
{
    let mut current = error.source();

    while let Some(source) = current {
        if let Some(io_error) = source.downcast_ref::<std_io::Error>() {
            return Some(io_error);
        }

        current = source.source();
    }

    None
}
