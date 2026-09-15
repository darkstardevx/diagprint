use serde::{Deserialize,Serialize}; use std::fmt;
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Serialize,Deserialize)]
#[serde(rename_all="lowercase")]
pub enum Severity{Trace,Debug,Info,Warning,Error,Fatal}
impl Severity{pub fn as_str(self)->&'static str{match self{Self::Trace=>"TRACE",Self::Debug=>"DEBUG",Self::Info=>"INFO",Self::Warning=>"WARNING",Self::Error=>"ERROR",Self::Fatal=>"FATAL"}}}
impl fmt::Display for Severity{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{f.write_str(self.as_str())}}
