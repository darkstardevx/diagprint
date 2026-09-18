mod option;
mod result;

#[cfg(feature = "futures")]
mod future;

#[cfg(feature = "futures")]
mod stream;

pub use option::DiagprintOptionExt;
pub use result::DiagprintResultExt;

#[cfg(feature = "futures")]
pub use future::DiagprintTryFutureExt;

#[cfg(feature = "futures")]
pub use stream::DiagprintTryStreamExt;
