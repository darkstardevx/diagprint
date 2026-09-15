use diagprint::{Applicability, Edit, FileCheck, FixError, FixPlan, FixPlanError, TextRange};
use std::{fs, path::PathBuf};
use uuid::Uuid;

fn temp_project(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("diagprint-fixplan-{name}-{}", Uuid::now_v7()))
}

fn replacement(file: &PathBuf, content: &str, from: &str, to: &str) -> Edit {
    let start = content.find(from).expect("test text exists");

    Edit::replace(file, TextRange::new(start, start + from.len()), from, to)
}

#[test]
fn multi_file_plan_applies_and_verifies() {
    let root = temp_project("multi");

    fs::create_dir_all(&root).unwrap();

    let first = root.join("first.txt");

    let second = root.join("second.txt");

    let first_original = "alpha=old\n";

    let second_original = "beta=old\n";

    fs::write(&first, first_original).unwrap();

    fs::write(&second, second_original).unwrap();

    let plan = FixPlan::new("update both files")
        .applicability(Applicability::MachineApplicable)
        .precondition(FileCheck::equals(&first, first_original))
        .precondition(FileCheck::equals(&second, second_original))
        .edit(replacement(&first, first_original, "old", "new"))
        .edit(replacement(&second, second_original, "old", "new"))
        .verify(FileCheck::contains(&first, "alpha=new"))
        .verify(FileCheck::contains(&second, "beta=new"));

    let check = plan.check().unwrap();

    assert_eq!(check.affected_files.len(), 2);

    assert_eq!(check.preconditions_checked, 2);

    assert_eq!(check.verifications_planned, 2);

    let report = plan.apply().unwrap();

    assert!(report.verified());

    assert_eq!(report.changed_files.len(), 2);

    assert_eq!(report.verification_checks, 2);

    assert_eq!(fs::read_to_string(&first,).unwrap(), "alpha=new\n");

    assert_eq!(fs::read_to_string(&second,).unwrap(), "beta=new\n");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_precondition_blocks_all_writes() {
    let root = temp_project("precondition");

    fs::create_dir_all(&root).unwrap();

    let file = root.join("config.txt");

    let original = "mode=old\n";

    fs::write(&file, original).unwrap();

    let plan = FixPlan::new("change mode")
        .applicability(Applicability::MachineApplicable)
        .precondition(FileCheck::contains(&file, "mode=expected"))
        .edit(replacement(&file, original, "old", "new"));

    let error = plan.apply().unwrap_err();

    assert!(matches!(error, FixPlanError::PreconditionsFailed { .. }));

    assert_eq!(fs::read_to_string(&file,).unwrap(), original);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_verification_rolls_back_all_files() {
    let root = temp_project("rollback");

    fs::create_dir_all(&root).unwrap();

    let first = root.join("first.txt");

    let second = root.join("second.txt");

    let first_original = "alpha=old\n";

    let second_original = "beta=old\n";

    fs::write(&first, first_original).unwrap();

    fs::write(&second, second_original).unwrap();

    let plan = FixPlan::new("transaction must roll back")
        .applicability(Applicability::MachineApplicable)
        .edit(replacement(&first, first_original, "old", "new"))
        .edit(replacement(&second, second_original, "old", "new"))
        .verify(FileCheck::contains(&first, "this will never exist"));

    let error = plan.apply().unwrap_err();

    match error {
        FixPlanError::VerificationFailed {
            failures,
            rollback_failures,
        } => {
            assert_eq!(failures.len(), 1);

            assert!(rollback_failures.is_empty());
        }

        other => {
            panic!("expected verification failure, got {other:?}");
        }
    }

    assert_eq!(fs::read_to_string(&first,).unwrap(), first_original);

    assert_eq!(fs::read_to_string(&second,).unwrap(), second_original);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_edit_is_rejected_before_any_file_is_written() {
    let root = temp_project("stale");

    fs::create_dir_all(&root).unwrap();

    let first = root.join("a.txt");

    let second = root.join("b.txt");

    let first_original = "first=old\n";

    let second_original = "second=current\n";

    fs::write(&first, first_original).unwrap();

    fs::write(&second, second_original).unwrap();

    let stale_second = Edit::replace(&second, TextRange::new(7, 10), "old", "new");

    let plan = FixPlan::new("stale transaction")
        .applicability(Applicability::MachineApplicable)
        .edit(replacement(&first, first_original, "old", "new"))
        .edit(stale_second);

    let error = plan.apply().unwrap_err();

    assert!(matches!(
        error,
        FixPlanError::Fix(FixError::StaleEdit { .. })
    ));

    assert_eq!(fs::read_to_string(&first,).unwrap(), first_original);

    assert_eq!(fs::read_to_string(&second,).unwrap(), second_original);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn manual_plan_cannot_apply() {
    let root = temp_project("manual");

    fs::create_dir_all(&root).unwrap();

    let file = root.join("config.txt");

    let original = "mode=old\n";

    fs::write(&file, original).unwrap();

    let plan = FixPlan::new("manual plan").edit(replacement(&file, original, "old", "new"));

    let error = plan.apply().unwrap_err();

    assert!(matches!(
        error,
        FixPlanError::NotMachineApplicable {
            applicability: Applicability::Manual
        }
    ));

    assert_eq!(fs::read_to_string(&file,).unwrap(), original);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn successful_plan_can_create_backups() {
    let root = temp_project("backup");

    fs::create_dir_all(&root).unwrap();

    let file = root.join("config.txt");

    let original = "mode=old\n";

    fs::write(&file, original).unwrap();

    let plan = FixPlan::new("backup plan")
        .applicability(Applicability::MachineApplicable)
        .backups(true)
        .edit(replacement(&file, original, "old", "new"))
        .verify(FileCheck::contains(&file, "mode=new"));

    plan.apply().unwrap();

    let backup = PathBuf::from(format!("{}.diagprint.bak", file.display()));

    assert_eq!(fs::read_to_string(backup,).unwrap(), original);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_describes_transaction_contract() {
    let root = temp_project("preview");

    fs::create_dir_all(&root).unwrap();

    let file = root.join("config.txt");

    let original = "mode=old\n";

    fs::write(&file, original).unwrap();

    let preview = FixPlan::new("preview plan")
        .explanation("show the whole remediation contract")
        .applicability(Applicability::MachineApplicable)
        .precondition(FileCheck::equals(&file, original))
        .edit(replacement(&file, original, "old", "new"))
        .verify(FileCheck::contains(&file, "mode=new"))
        .preview()
        .render();

    assert!(preview.contains("FIX PLAN  preview plan"));

    assert!(preview.contains("PRECONDITION"));

    assert!(preview.contains("PATCH"));

    assert!(preview.contains("VERIFY"));

    assert!(preview.contains("rollback-on-error enabled"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn machine_applicable_plan_rejects_unguarded_insert() {
    let root = temp_project("unguarded-insert");

    fs::create_dir_all(&root).unwrap();

    let file = root.join("config.txt");

    fs::write(&file, "mode=old\n").unwrap();

    let plan = FixPlan::new("unsafe insert")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::insert(&file, 0, "prefix\n"));

    let error = plan.apply().unwrap_err();

    assert!(matches!(error, FixPlanError::UnguardedInsert { .. }));

    assert_eq!(fs::read_to_string(&file,).unwrap(), "mode=old\n");

    fs::remove_dir_all(root).unwrap();
}
