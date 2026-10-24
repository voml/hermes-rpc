//! `hermes-generator` 的生成计划、渲染与落盘测试。

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use hermes_generator::{FileOwnership, GeneratorError, generate, generate_from_json};
use vos_types::{CanonicalType, OperationProjectionArtifact};

/// 三个 operation：带参数的 query、带返回值的 udf、无返回值的 micro。
///
/// span 与 `sourceUnits[0].source` 的字节位置一一对应，保证 artifact 通过校验。
const FIXTURE: &str = r#"{
  "envelope": {
    "stage": "operation-syntax-projection",
    "contractFormatVersion": "vos-operation-contract-v0",
    "languageVersion": "vos-language-v0",
    "canonicalizationVersion": "vos-canonical-v0",
    "schemaFingerprint": null,
    "sourceUnits": [{
      "sourceUnitId": 0,
      "source": "query Find(a: int, b: utf8): bool\nudf Ping(): utf8\nmicro Reset()"
    }]
  },
  "operations": [
    {
      "kind": "query",
      "name": "Find",
      "identityKey": "query:Find(a:int,b:utf8)->bool",
      "operationId": "op_9fd56a2e3c3df8d8",
      "parameters": [
        {"name": "a", "typeExpr": {"named": ["int"]}, "span": {"start": 11, "end": 12}},
        {"name": "b", "typeExpr": {"named": ["utf8"]}, "span": {"start": 19, "end": 20}}
      ],
      "returnType": {"named": ["bool"]},
      "signature": null,
      "body": null,
      "span": {"start": 0, "end": 33}
    },
    {
      "kind": "udf",
      "name": "Ping",
      "identityKey": "udf:Ping()->utf8",
      "operationId": "op_4499bae698e1e66b",
      "parameters": [],
      "returnType": {"named": ["utf8"]},
      "signature": null,
      "body": null,
      "span": {"start": 34, "end": 50}
    },
    {
      "kind": "micro",
      "name": "Reset",
      "identityKey": "micro:Reset()",
      "operationId": "op_047d2ef6137266b7",
      "parameters": [],
      "returnType": null,
      "signature": null,
      "body": null,
      "span": {"start": 51, "end": 64}
    }
  ]
}"#;

fn parsed_artifact() -> OperationProjectionArtifact {
    OperationProjectionArtifact::from_json(FIXTURE).expect("fixture should be a valid artifact")
}

fn contents_of<'a>(plan: &'a hermes_generator::GenerationPlan, path: &str) -> &'a str {
    plan.files().iter().find(|file| file.path() == path).unwrap_or_else(|| panic!("missing {path}")).contents()
}

fn temp_root(tag: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after epoch").as_nanos();
    let root = std::env::temp_dir().join(format!("hermes-generator-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

// --- 计划形状 -----------------------------------------------------------

#[test]
fn plan_contains_generated_client_server_and_one_template() {
    let plan = generate_from_json(FIXTURE).expect("generation succeeds");

    assert_eq!(plan.files().len(), 3);
    assert_eq!(contents_of(&plan, "hermes/operations.ts").is_empty(), false);

    let ownership: Vec<_> = plan.files().iter().map(|file| (file.path(), file.ownership())).collect();
    assert_eq!(
        ownership,
        vec![
            ("hermes/operations.ts", FileOwnership::Generated),
            ("hermes/server.ts", FileOwnership::Generated),
            ("hermes/handlers.ts", FileOwnership::Template),
        ]
    );
    assert!(plan.diagnostics().is_empty());
}

#[test]
fn generation_is_deterministic() {
    let first = generate(&parsed_artifact()).expect("first generation");
    let second = generate(&parsed_artifact()).expect("second generation");

    let first_files: Vec<_> = first.files().iter().map(|file| (file.path(), file.contents())).collect();
    let second_files: Vec<_> = second.files().iter().map(|file| (file.path(), file.contents())).collect();
    assert_eq!(first_files, second_files);
}

// --- client 渲染 --------------------------------------------------------

#[test]
fn client_maps_operation_ids_parameters_and_results() {
    let plan = generate(&parsed_artifact()).expect("generation succeeds");
    let client = contents_of(&plan, "hermes/operations.ts");

    assert!(client.contains("  find: \"op_9fd56a2e3c3df8d8\","));
    assert!(client.contains("  ping: \"op_4499bae698e1e66b\","));
    // `int` 是未定宽整数，wire 上按字符串承载以避免 JS number 精度丢失
    assert!(client.contains("export type FindParams = {\n  a: string;\n  b: string;\n};"));
    assert!(client.contains("export type PingParams = Record<string, never>;"));
    assert!(client.contains("export type FindResult = boolean;"));
    assert!(client.contains("export type PingResult = string;"));
    assert!(client.contains("export type ResetResult = void;"));
    assert!(client.contains("  find(params: FindParams): Promise<FindResult>;"));
    assert!(client.contains("transport.dispatch(operationIds.find, params)"));
    // 生成物不得 import Hermes npm 包
    assert!(client.contains("export interface HermesTransport"));
    assert!(!client.contains("@hermes/"));
}

// --- server 渲染与模板 --------------------------------------------------

#[test]
fn server_binds_handlers_by_operation_id() {
    let plan = generate(&parsed_artifact()).expect("generation succeeds");
    let server = contents_of(&plan, "hermes/server.ts");

    assert!(server.contains(
        "import type { FindParams, FindResult, PingParams, PingResult, ResetParams, ResetResult } from \"./operations\";"
    ));
    assert!(server.contains("  find(ctx: HermesHandlerContext, params: FindParams): Promise<FindResult> | FindResult;"));
    assert!(server.contains("      operationId: \"op_9fd56a2e3c3df8d8\","));
    assert!(server.contains("handler: (ctx, params) => handlers.find(ctx, params as FindParams),"));
}

#[test]
fn template_is_one_time_and_marks_unimplemented_operations() {
    let plan = generate(&parsed_artifact()).expect("generation succeeds");
    let template = contents_of(&plan, "hermes/handlers.ts");

    assert!(template.contains("This file is never overwritten."));
    assert!(template.contains("throw new Error(\"operation `Reset` is not implemented\");"));
}

// --- 诊断与冲突 --------------------------------------------------------

#[test]
fn unresolved_named_type_falls_back_to_unknown_with_diagnostic() {
    let mut artifact = parsed_artifact();
    artifact.operations[2].return_type = Some(CanonicalType::Named(vec!["U".to_string()]));

    let plan = generate(&artifact).expect("generation still succeeds");
    let client = contents_of(&plan, "hermes/operations.ts");

    assert!(client.contains("export type ResetResult = unknown;"));
    assert!(
        plan.diagnostics().iter().any(|diagnostic| diagnostic.message().contains("operation `Reset` return type `U`")),
        "expected diagnostic for unresolved named type, got {:?}",
        plan.diagnostics()
    );
}

#[test]
fn naming_collision_is_rejected() {
    let mut artifact = parsed_artifact();
    artifact.operations[0].name = "a-b".into();
    artifact.operations[1].name = "a.b".into();

    match generate(&artifact).expect_err("collision must be rejected") {
        GeneratorError::NamingCollision { identifier, operations } => {
            assert_eq!(identifier, "AB");
            assert_eq!(operations, vec!["a-b".to_string(), "a.b".to_string()]);
        }
        other => panic!("expected NamingCollision, got {other}"),
    }
}

// --- 落盘 --------------------------------------------------------------

#[test]
fn write_to_creates_files_and_preserves_existing_template() {
    let root = temp_root("write");
    let plan = generate(&parsed_artifact()).expect("generation succeeds");

    let first = plan.write_to(&root).expect("first write");
    assert_eq!(first.written().len(), 3);
    assert!(first.skipped().is_empty());

    let template_path = root.join("hermes/handlers.ts");
    fs::write(&template_path, "// user implementation\n").expect("overwrite template with user code");

    let second = plan.write_to(&root).expect("second write");
    assert!(second.written().is_empty(), "nothing changed, got {:?}", second.written());
    assert_eq!(second.unchanged(), &["hermes/operations.ts".to_string(), "hermes/server.ts".to_string()]);
    assert_eq!(second.skipped(), &["hermes/handlers.ts".to_string()]);
    assert_eq!(fs::read_to_string(&template_path).expect("template readable"), "// user implementation\n");

    fs::remove_dir_all(&root).expect("cleanup temp root");
}

#[test]
fn generated_file_is_rewritten_when_content_changes() {
    let root = temp_root("rewrite");
    let plan = generate(&parsed_artifact()).expect("generation succeeds");
    plan.write_to(&root).expect("first write");

    let client_path: &Path = &root.join("hermes/operations.ts");
    fs::write(client_path, "// stale\n").expect("write stale content");

    let report = plan.write_to(&root).expect("second write");
    assert_eq!(report.written(), &["hermes/operations.ts".to_string()]);
    assert!(fs::read_to_string(client_path).expect("client readable").contains("createHermesClient"));

    fs::remove_dir_all(&root).expect("cleanup temp root");
}

// --- 空 artifact -------------------------------------------------------

#[test]
fn empty_artifact_still_produces_parseable_stub_modules() {
    const EMPTY: &str = r#"{
      "envelope": {
        "stage": "operation-syntax-projection",
        "contractFormatVersion": "vos-operation-contract-v0",
        "languageVersion": "vos-language-v0",
        "canonicalizationVersion": "vos-canonical-v0",
        "schemaFingerprint": null,
        "sourceUnits": [{"sourceUnitId": 0, "source": ""}]
      },
      "operations": []
    }"#;

    let plan = generate_from_json(EMPTY).expect("empty artifact is valid");
    assert!(contents_of(&plan, "hermes/operations.ts").contains("export const operationIds = {\n} as const;"));
    assert!(contents_of(&plan, "hermes/server.ts").contains("export {};"));
    assert!(contents_of(&plan, "hermes/handlers.ts").contains("export {};"));
}
