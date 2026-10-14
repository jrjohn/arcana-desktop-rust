//! C# `AppErrorTests` (8) and `ResultTests` (11). C# `Result<T>` is `AppResult<T>`
//! (`Match` → `match`, `Map` → `map`, `Bind` → `and_then`, `GetValueOrDefault` → `unwrap_or`,
//! `OnSuccess`/`OnFailure` → `inspect`/`inspect_err`, implicit conversions → `Ok`/`Err`/`?`).

use std::hint::black_box;

use arcana_core::{AppError, AppResult, ErrorCode};

fn auth(code: ErrorCode, message: &str) -> AppError {
    AppError::Auth {
        code,
        message: message.into(),
    }
}

// NetworkError_ToUserMessage_ShouldReturnMessage
#[test]
fn network_error_to_user_message_should_return_message() {
    let e = AppError::Network {
        code: ErrorCode::NetworkUnavailable,
        message: "No network".into(),
        inner: None,
    };
    assert_eq!(e.to_user_message(), "No network");
    assert_eq!(e.code(), ErrorCode::NetworkUnavailable);
}

// ValidationError_WithErrors_ToUserMessage_ShouldJoinErrors
#[test]
fn validation_error_with_errors_to_user_message_should_join_errors() {
    let e = AppError::validation("Invalid", vec!["Field required".into(), "Too short".into()]);
    let msg = e.to_user_message();
    assert!(msg.contains("Field required") && msg.contains("Too short"));
    assert!(matches!(e, AppError::Validation { ref errors, .. } if errors.len() == 2));
}

// ValidationError_EmptyErrors_ToUserMessage_ShouldReturnMessage
#[test]
fn validation_error_empty_errors_to_user_message_should_return_message() {
    assert_eq!(
        AppError::validation("Invalid input", vec![]).to_user_message(),
        "Invalid input"
    );
}

// ServerError_ShouldHaveStatusCode
#[test]
fn server_error_should_have_status_code() {
    let e = AppError::Server {
        code: ErrorCode::ServerError,
        message: "Server error".into(),
        status_code: Some(500),
    };
    assert!(matches!(
        e,
        AppError::Server {
            status_code: Some(500),
            ..
        }
    ));
    assert_eq!(e.to_user_message(), "Server error");
}

// AuthError_ShouldSetProperties
#[test]
fn auth_error_should_set_properties() {
    let e = auth(ErrorCode::AuthenticationFailed, "Invalid credentials");
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::AuthenticationFailed, "Invalid credentials")
    );
}

// PluginError_ShouldSetPluginId
#[test]
fn plugin_error_should_set_plugin_id() {
    let e = AppError::Plugin {
        code: ErrorCode::PluginNotFound,
        message: "Plugin missing".into(),
        plugin_id: Some("arcana.order".into()),
    };
    assert!(matches!(e, AppError::Plugin { plugin_id: Some(ref p), .. } if p == "arcana.order"));
    assert_eq!(e.code(), ErrorCode::PluginNotFound);
}

// UnknownError_WithException_ShouldSetInnerException
#[test]
fn unknown_error_with_exception_should_set_inner_exception() {
    let e = AppError::Unknown {
        message: "Unexpected error".into(),
        inner: Some("cause".into()),
    };
    assert_eq!(e.code(), ErrorCode::Unknown);
    assert!(matches!(e, AppError::Unknown { inner: Some(ref i), .. } if i == "cause"));
}

// DataError_ShouldSetProperties
#[test]
fn data_error_should_set_properties() {
    let e = AppError::data(ErrorCode::DatabaseError, "DB failed");
    assert_eq!(e.code(), ErrorCode::DatabaseError);
    assert_eq!(e.to_user_message(), "DB failed");
}

// Success_ShouldSetValueAndIsSuccess
#[test]
fn success_should_set_value_and_is_success() {
    let r: AppResult<i32> = black_box(Ok(42));
    assert!(r.is_ok());
    assert!(r.as_ref().err().is_none());
    assert_eq!(r.as_ref().ok(), Some(&42));
    assert!(r.err().is_none());
}

// Failure_ShouldSetErrorAndIsFailure
#[test]
fn failure_should_set_error_and_is_failure() {
    let e = auth(ErrorCode::AuthenticationFailed, "Unauthorized");
    let r: AppResult<String> = Err(e.clone());
    assert!(r.is_err());
    assert_eq!(r.err(), Some(e));
}

// Match_OnSuccess_ShouldCallSuccessHandler
#[test]
fn match_on_success_should_call_success_handler() {
    let r: AppResult<i32> = Ok(10);
    assert_eq!(r.map_or(-1, |v| v * 2), 20);
}

// Match_OnFailure_ShouldCallFailureHandler
#[test]
fn match_on_failure_should_call_failure_handler() {
    let r: AppResult<i32> = black_box(Err(AppError::Network {
        code: ErrorCode::NetworkUnavailable,
        message: "No network".into(),
        inner: None,
    }));
    let out = r.unwrap_or(-99);
    assert_eq!(out, -99);
}

// Map_OnSuccess_ShouldTransformValue
#[test]
fn map_on_success_should_transform_value() {
    let r: AppResult<i32> = Ok(5);
    assert_eq!(r.map(|v| v.to_string()).unwrap(), "5");
}

// Bind_OnSuccess_ShouldChain
#[test]
fn bind_on_success_should_chain() {
    let r: AppResult<i32> = Ok(5);
    assert_eq!(
        r.and_then(|v| black_box(Ok::<_, AppError>(format!("val={v}"))))
            .unwrap(),
        "val=5"
    );
}

// GetValueOrDefault_OnFailure_ShouldReturnDefault
#[test]
fn get_value_or_default_on_failure_should_return_default() {
    let r: AppResult<i32> = black_box(Err(auth(ErrorCode::Unauthorized, "Forbidden")));
    assert_eq!(r.unwrap_or(99), 99);
}

// OnSuccess_ShouldInvokeAction
#[test]
fn on_success_should_invoke_action() {
    let mut called = false;
    let _ = Ok::<i32, AppError>(1).inspect(|_| called = true);
    assert!(called);
}

// OnFailure_ShouldInvokeAction
#[test]
fn on_failure_should_invoke_action() {
    let mut called = false;
    let _ =
        Err::<i32, _>(auth(ErrorCode::AuthenticationFailed, "Fail")).inspect_err(|_| called = true);
    assert!(called);
}

// ImplicitConversion_FromValue_ShouldCreateSuccess
#[test]
fn implicit_conversion_from_value_should_create_success() {
    fn make() -> AppResult<String> {
        black_box(Ok("hello".into()))
    }
    assert_eq!(make().unwrap(), "hello");
}

// ImplicitConversion_FromError_ShouldCreateFailure
#[test]
fn implicit_conversion_from_error_should_create_failure() {
    fn inner() -> AppResult<()> {
        Err(auth(ErrorCode::Forbidden, "No access"))
    }
    fn outer() -> AppResult<i32> {
        inner()?;
        Ok(1)
    }
    assert_eq!(outer().unwrap_err().code(), ErrorCode::Forbidden);
}
