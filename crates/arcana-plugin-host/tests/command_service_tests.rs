//! C# `CommandServiceTests`: 25 methods / 31 cases (two Theories: 3 + 5 rows).
//! Ported 23 methods / 29 cases. Not ported: `RegisterCommand_ShouldLogDebugMessage` and
//! `Constructor_WithLogger_ShouldWork` (no injected `ILogger`; the impl logs via `tracing`).

use std::any::Any;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use arcana_core::{AppError, AppResult};
use arcana_plugin_api::{CommandArgs, CommandHandler, CommandService};
use arcana_plugin_host::services::command::CommandServiceImpl;

fn noop() -> CommandHandler {
    Arc::new(|_: &CommandArgs| Ok(()))
}

fn args(v: Vec<Box<dyn Any + Send>>) -> CommandArgs {
    v
}

fn is_validation(e: &AppError) -> bool {
    matches!(e, AppError::Validation { .. })
}
fn is_invalid_op(e: &AppError) -> bool {
    matches!(e, AppError::Plugin { .. })
}

// RegisterCommand_ValidCommand_ShouldRegisterSuccessfully
#[test]
fn register_valid() {
    let s = CommandServiceImpl::new();
    let _sub = s.register_command("test.command", noop()).unwrap();
    assert!(s.has_command("test.command"));
}

// RegisterCommand_DuplicateId_ShouldThrowInvalidOperationException
#[test]
fn register_duplicate_fails() {
    let s = CommandServiceImpl::new();
    let _a = s.register_command("test.command", noop()).unwrap();
    let e = s.register_command("test.command", noop()).unwrap_err();
    assert!(is_invalid_op(&e));
}

// RegisterCommand_InvalidId_ShouldThrowContributionValidationException
#[test]
fn register_invalid_id() {
    let e = CommandServiceImpl::new()
        .register_command("123invalid", noop())
        .unwrap_err();
    assert!(matches!(e, AppError::Validation { ref errors, .. } if !errors.is_empty()));
}

// RegisterCommand_EmptyOrNullId_ShouldThrowContributionValidationException ×3 ("", "   ",
// null → "": Rust `&str` cannot be null)
#[test]
fn register_empty_id() {
    for id in ["", "   ", ""] {
        let e = CommandServiceImpl::new()
            .register_command(id, noop())
            .unwrap_err();
        assert!(
            matches!(&e, AppError::Validation { errors, .. } if errors.iter().any(|m| m.contains("Id is required"))),
            "{id:?}"
        );
    }
}

// RegisterCommand_ValidIdFormats_ShouldRegisterSuccessfully ×5
#[test]
fn register_valid_formats() {
    for id in [
        "file.new",
        "order.list",
        "app.exit",
        "command_test",
        "command-test",
    ] {
        let s = CommandServiceImpl::new();
        let _sub = s.register_command(id, noop()).unwrap();
        assert!(s.has_command(id), "{id}");
    }
}

// RegisterCommand_Typed_ShouldRegisterSuccessfully
#[test]
fn register_typed() {
    let s = CommandServiceImpl::new();
    let _sub = s.register_typed::<i32>("test.command", |_| Ok(())).unwrap();
    assert!(s.has_command("test.command"));
}

// RegisterCommand_Typed_ShouldPassArgumentCorrectly
#[test]
fn register_typed_passes_argument() {
    let s = CommandServiceImpl::new();
    let got = Arc::new(AtomicI32::new(0));
    let g = got.clone();
    let _sub = s
        .register_typed::<i32>("test.command", move |a| {
            g.store(a.copied().unwrap_or_default(), Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
    s.execute("test.command", args(vec![Box::new(42_i32)]))
        .unwrap();
    assert_eq!(got.load(Ordering::SeqCst), 42);
}

// RegisterCommand_Typed_ShouldHandleDefaultWhenNoArgument
#[test]
fn register_typed_default_without_argument() {
    let s = CommandServiceImpl::new();
    let got = Arc::new(AtomicI32::new(-1));
    let g = got.clone();
    let _sub = s
        .register_typed::<i32>("test.command", move |a| {
            g.store(a.copied().unwrap_or_default(), Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
    s.execute("test.command", Vec::new()).unwrap();
    assert_eq!(got.load(Ordering::SeqCst), 0);
}

// ExecuteAsync_RegisteredCommand_ShouldExecuteHandler
#[test]
fn execute_runs_handler() {
    let s = CommandServiceImpl::new();
    let n = Arc::new(AtomicUsize::new(0));
    let n2 = n.clone();
    let _sub = s
        .register_command(
            "test.command",
            Arc::new(move |_: &CommandArgs| {
                n2.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        )
        .unwrap();
    s.execute("test.command", Vec::new()).unwrap();
    assert_eq!(n.load(Ordering::SeqCst), 1);
}

// ExecuteAsync_WithArguments_ShouldPassArguments
#[test]
fn execute_passes_arguments() {
    let s = CommandServiceImpl::new();
    let seen = Arc::new(Mutex::new(None));
    let seen2 = seen.clone();
    let _sub = s
        .register_command(
            "test.command",
            Arc::new(move |a: &CommandArgs| -> AppResult<()> {
                *seen2.lock().unwrap() = Some((
                    a.len(),
                    a[0].downcast_ref::<&str>().copied(),
                    a[1].downcast_ref::<i32>().copied(),
                    a[2].downcast_ref::<bool>().copied(),
                ));
                Ok(())
            }),
        )
        .unwrap();
    s.execute(
        "test.command",
        args(vec![Box::new("arg1"), Box::new(42_i32), Box::new(true)]),
    )
    .unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        Some((3, Some("arg1"), Some(42), Some(true)))
    );
}

// ExecuteAsync_NonExistingCommand_ShouldThrowInvalidOperationException
#[test]
fn execute_unknown_fails() {
    let e = CommandServiceImpl::new()
        .execute("non.existing.command", Vec::new())
        .unwrap_err();
    assert!(is_invalid_op(&e));
    assert!(e.message().contains("Command not found"));
}

// ExecuteAsync_ShouldReturnNull — Rust returns `Ok(())`.
#[test]
fn execute_returns_unit() {
    let s = CommandServiceImpl::new();
    let _sub = s.register_command("test.command", noop()).unwrap();
    assert_eq!(s.execute("test.command", Vec::new()).ok(), Some(()));
}

// GetCommands_ShouldReturnAllRegisteredCommands
#[test]
fn commands_lists_all() {
    let s = CommandServiceImpl::new();
    let _subs: Vec<_> = ["command1", "command2", "command3"]
        .iter()
        .map(|c| s.register_command(c, noop()).unwrap())
        .collect();
    assert_eq!(s.commands(), ["command1", "command2", "command3"]);
}

// GetCommands_NoRegistrations_ShouldReturnEmptyList
#[test]
fn commands_empty() {
    assert_eq!(CommandServiceImpl::new().commands().len(), 0);
}

// HasCommand_RegisteredCommand_ShouldReturnTrue
#[test]
fn has_command_true() {
    let s = CommandServiceImpl::new();
    let _sub = s.register_command("test.command", noop()).unwrap();
    assert!(s.has_command("test.command"));
}

// HasCommand_NonRegisteredCommand_ShouldReturnFalse
#[test]
fn has_command_false() {
    assert!(!CommandServiceImpl::new().has_command("non.existing"));
}

// HasCommand_AfterUnregister_ShouldReturnFalse
#[test]
fn has_command_after_unregister() {
    let s = CommandServiceImpl::new();
    s.register_command("test.command", noop())
        .unwrap()
        .dispose();
    assert!(!s.has_command("test.command"));
}

// Dispose_Subscription_ShouldRemoveCommand
#[test]
fn dispose_removes() {
    let s = CommandServiceImpl::new();
    let sub = s.register_command("test.command", noop()).unwrap();
    sub.dispose();
    assert!(!s.has_command("test.command"));
    assert_eq!(s.commands().len(), 0);
}

// Dispose_Subscription_ExecuteAsync_ShouldThrow
#[test]
fn dispose_then_execute_fails() {
    let s = CommandServiceImpl::new();
    s.register_command("test.command", noop())
        .unwrap()
        .dispose();
    assert!(is_invalid_op(
        &s.execute("test.command", Vec::new()).unwrap_err()
    ));
}

// Dispose_Subscription_MultipleTimes_ShouldNotThrow — a second dispose is a compile error in
// Rust (`dispose(self)`); re-registering after dispose must work instead.
#[test]
fn dispose_then_reregister() {
    let s = CommandServiceImpl::new();
    s.register_command("test.command", noop())
        .unwrap()
        .dispose();
    assert!(s.register_command("test.command", noop()).is_ok());
}

// Constructor_WithoutLogger_ShouldWork
#[test]
fn constructor() {
    let s = CommandServiceImpl::default();
    assert_eq!(s.commands().len(), 0);
}

// ConcurrentRegistration_ShouldBeThreadSafe
#[test]
fn concurrent_registration() {
    let s = CommandServiceImpl::new();
    std::thread::scope(|sc| {
        for i in 0..100 {
            let s = &s;
            sc.spawn(move || {
                let _ = s.register_command(&format!("command{i}"), noop());
            });
        }
    });
    assert_eq!(s.commands().len(), 100);
}

// ConcurrentExecution_ShouldBeThreadSafe
#[test]
fn concurrent_execution() {
    let s = CommandServiceImpl::new();
    let n = Arc::new(AtomicUsize::new(0));
    let n2 = n.clone();
    let _sub = s
        .register_command(
            "test.command",
            Arc::new(move |_: &CommandArgs| {
                n2.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        )
        .unwrap();
    std::thread::scope(|sc| {
        for _ in 0..100 {
            sc.spawn(|| s.execute("test.command", Vec::new()));
        }
    });
    assert_eq!(n.load(Ordering::SeqCst), 100);
    let _ = is_validation;
}
