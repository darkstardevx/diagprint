use diagprint::{
    DiagnosticReport, DiagnosticSink, JsonLinesSink, Reporter, WriterSink, render::PlainRenderer,
};

#[test]
fn writer_sink_emits_reports() {
    let reporter = Reporter::builder()
        .application("sink-test")
        .build()
        .unwrap();

    let report: DiagnosticReport = [reporter.info("first"), reporter.error("second")]
        .into_iter()
        .collect();

    let sink = WriterSink::new(Vec::<u8>::new(), PlainRenderer);

    assert_eq!(sink.emit_report(&report).unwrap(), 2);

    sink.flush().unwrap();

    let bytes = sink.into_inner().unwrap();

    let text = String::from_utf8(bytes).unwrap();

    assert!(text.contains("first"));

    assert!(text.contains("second"));
}

#[test]
fn json_lines_sink_emits_one_object_per_line() {
    let reporter = Reporter::builder()
        .application("jsonl-test")
        .build()
        .unwrap();

    let sink = JsonLinesSink::new(Vec::<u8>::new());

    sink.emit(&reporter.info("one")).unwrap();

    sink.emit(&reporter.warning("two")).unwrap();

    let bytes = sink.into_inner().unwrap();

    let text = String::from_utf8(bytes).unwrap();

    let lines = text.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 2);

    for line in lines {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();

        assert!(value.is_object());
    }
}
