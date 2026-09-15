use super::Renderer;use crate::Diagnostic;
#[derive(Debug,Default,Clone,Copy)]pub struct JsonRenderer;
impl Renderer for JsonRenderer{fn render(&self,d:&Diagnostic)->String{serde_json::to_string_pretty(d).expect("diagnostic serialization failed")}}
