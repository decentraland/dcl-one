use crate::js::errors::OpError;
use crate::js::plain::JsPlain;
use dcl_js::testing::{
    SceneTestPlan, SceneTestResult, TakeAndCompareSnapshotResponse,
    TestingScreenshotComparisonMethodRequest,
};
use deno_core::{op2, OpDecl, OpState};

use crate::js::plain::Plain;

#[derive(Default)]
pub struct TestPlan {
    pub tests: Vec<String>,
}

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![
        op_testing_enabled(),
        op_take_and_compare_snapshot(),
        op_log_test_result(),
        op_log_test_plan(),
    ]
}

#[op2(fast)]
fn op_testing_enabled(op_state: &mut OpState) -> bool {
    dcl_js::testing::op_testing_enabled(op_state)
}

#[op2]
fn op_log_test_plan(
    state: &mut OpState,
    #[scoped] body: Plain<SceneTestPlan>,
) -> Result<(), OpError> {
    Ok(dcl_js::testing::op_log_test_plan(state, body.0)?)
}

#[op2]
fn op_log_test_result(
    state: &mut OpState,
    #[scoped] body: Plain<SceneTestResult>,
) -> Result<(), OpError> {
    Ok(dcl_js::testing::op_log_test_result(state, body.0)?)
}

#[op2]
fn op_take_and_compare_snapshot(
    state: &mut OpState,
    #[string] name: String,
    #[scoped] camera_position: Plain<(f32, f32, f32)>,
    #[scoped] camera_target: Plain<(f32, f32, f32)>,
    #[scoped] snapshot_size: Plain<(u32, u32)>,
    #[scoped] method: Plain<TestingScreenshotComparisonMethodRequest>,
) -> Result<JsPlain<TakeAndCompareSnapshotResponse>, OpError> {
    Ok(JsPlain(dcl_js::testing::op_take_and_compare_snapshot(
        state,
        name,
        camera_position.0,
        camera_target.0,
        snapshot_size.0,
        method.0,
    )?))
}
