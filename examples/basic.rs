use diagprint::{Reporter,RotationCadence,Severity};
fn main()->diagprint::Result<()>{
 let r=Reporter::builder().application("omniscient").min_severity(Severity::Info).file("reports/omniscient.log").max_file_size(1024*1024).rotation_count(5).rotation_cadence(RotationCadence::Daily).show_metadata(true).width(82).build()?;
 let d=r.error("Network initialization failed").code("NET-001").label("examples/basic.rs",4,Some(8),Some(8),Some("diagnostic creation")).cause("failed to open network interface").cause("permission denied").note("Fallback interface was not selected").help("Check interface permissions and driver state");
 r.emit(&d)?;Ok(())
}
