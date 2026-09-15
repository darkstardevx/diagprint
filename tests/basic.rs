use diagprint::{render::{JsonRenderer,Renderer},Cause,Reporter,Severity};
#[test]fn sessions_unique(){let a=Reporter::builder().build().unwrap();let b=Reporter::builder().build().unwrap();assert_ne!(a.session_id(),b.session_id())}
#[test]fn filter(){let r=Reporter::builder().min_severity(Severity::Warning).color(false).build().unwrap();assert!(!r.emit(&r.info("no")).unwrap());assert!(r.emit(&r.warning("yes")).unwrap())}
#[test]fn json(){let r=Reporter::builder().application("test").build().unwrap();let s=JsonRenderer.render(&r.error("boom").code("E1"));assert!(s.contains("report_id"));assert!(s.contains("E1"))}
#[test]fn nested_cause(){let c=Cause::new("one").caused_by(Cause::new("two").caused_by(Cause::new("three")));assert_eq!(c.iter().count(),3)}
#[test]fn width_floor(){let r=Reporter::builder().width(10).color(false).build().unwrap();assert!(r.emit(&r.error("works")).unwrap())}
