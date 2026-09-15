use diagprint::Reporter;use std::{error::Error,fmt};
#[derive(Debug)]struct Inner;impl fmt::Display for Inner{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{write!(f,"permission denied")}}impl Error for Inner{}
#[derive(Debug)]struct Outer{source:Inner}impl fmt::Display for Outer{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{write!(f,"failed to open interface")}}impl Error for Outer{fn source(&self)->Option<&(dyn Error+'static)>{Some(&self.source)}}
fn main()->diagprint::Result<()>{let r=Reporter::builder().application("error-demo").build()?;let e=Outer{source:Inner};let d=r.error("Network initialization failed").code("NET-001").from_error(&e);r.emit(&d)?;Ok(())}
