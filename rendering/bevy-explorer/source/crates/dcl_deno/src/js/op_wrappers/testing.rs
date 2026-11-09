use dcl_js::testing::{
    SceneTestPlan, SceneTestResult, TakeAndCompareSnapshotResponse,
    TestingScreenshotComparisonMethodRequest,
};
use deno_core::{error::AnyError, op2, OpDecl, OpState};

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
    #[from_v8] body: Plain<SceneTestPlan>,
) -> Result<(), AnyError> {
    dcl_js::testing::op_log_test_plan(state, body.0)
}

#[op2]
fn op_log_test_result(
    state: &mut OpState,
    #[from_v8] body: Plain<SceneTestResult>,
) -> Result<(), AnyError> {
    dcl_js::testing::op_log_test_result(state, body.0)
}

#[op2]
#[serde]
fn op_take_and_compare_snapshot(
    state: &mut OpState,
    #[string] name: String,
    #[from_v8] camera_position: Plain<(f32, f32, f32)>,
    #[from_v8] camera_target: Plain<(f32, f32, f32)>,
    #[from_v8] snapshot_size: Plain<(u32, u32)>,
    #[from_v8] method: Plain<TestingScreenshotComparisonMethodRequest>,
) -> Result<TakeAndCompareSnapshotResponse, AnyError> {
    dcl_js::testing::op_take_and_compare_snapshot(
        state,
        name,
        camera_position.0,
        camera_target.0,
        snapshot_size.0,
        method.0,
    )
}
