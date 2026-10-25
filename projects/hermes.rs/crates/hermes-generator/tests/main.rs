//! `hermes-generator` 的生成计划、渲染与落盘测试。
//!
//! Fixture 直接由上游 `vos-types` 的投影类型构造：`identity` 是结构化 key，
//! `operationId` 是 64 hex 的 durable id，二者都由上游规则派生，测试不手拼。

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use hermes_generator::{AotTarget, FileOwnership, GeneratorError, generate, generate_from_json, generate_service};
use vos_types::{
    CANONICALIZATION_VERSION, CanonicalType, ContractArtifact, ContractEnvelope, ContractFormat, LANGUAGE_VERSION,
    OPERATION_CONTRACT_FORMAT_VERSION, OPERATION_PROJECTION_STAGE, OperationIdentityKey, OperationKind, OperationParameter,
    OperationParameterKey, OperationProjection, OperationProjectionArtifact, SourceUnit, canonical_type_key,
};

const SOURCE: &str = "query Find(a: int, b: utf8): bool\nudf Ping(): utf8\nmicro Reset()";
const METHOD_FIXTURE: &str = include_str!("fixtures/identity_service.contract.json");

fn span_of(needle: &str) -> std::ops::Range<usize> {
    let start = SOURCE.find(needle).unwrap_or_else(|| panic!("source contains {needle}"));
    start..start + needle.len()
}

fn operation(
    kind: OperationKind,
    name: &str,
    parameters: Vec<(&str, CanonicalType, std::ops::Range<usize>)>,
    return_type: Option<CanonicalType>,
    span: std::ops::Range<usize>,
) -> OperationProjection {
    let parameter_keys = parameters
        .iter()
        .map(|(name, ty, _)| OperationParameterKey::new(name, &canonical_type_key(ty)).expect("parameter key"))
        .collect::<Vec<_>>();
    let result_key = return_type.as_ref().map(canonical_type_key).unwrap_or_else(|| "unit".to_string());
    let identity =
        OperationIdentityKey::new(&[name.to_string()], kind, name, parameter_keys, &result_key).expect("operation identity");
    OperationProjection {
        kind,
        name: name.to_string(),
        canonical_path: identity.canonical_path.clone(),
        operation_id: vos_types::OperationId::derive(&identity),
        identity,
        parameters: parameters
            .into_iter()
            .map(|(name, type_expr, span)| OperationParameter { name: name.to_string(), type_expr, span: span.into() })
            .collect(),
        return_type,
        signature: None,
        body: None,
        span: span.into(),
    }
}

fn parsed_artifact() -> OperationProjectionArtifact {
    OperationProjectionArtifact {
        envelope: ContractEnvelope {
            stage: OPERATION_PROJECTION_STAGE.to_string(),
            contract_format_version: OPERATION_CONTRACT_FORMAT_VERSION.to_string(),
            language_version: LANGUAGE_VERSION.to_string(),
            canonicalization_version: CANONICALIZATION_VERSION.to_string(),
            schema_fingerprint: None,
            source_units: vec![SourceUnit { source_unit_id: 0, source: SOURCE.to_string() }],
        },
        operations: vec![
            operation(
                OperationKind::Query,
                "Find",
                vec![
                    ("a", CanonicalType::Named(vec!["int".to_string()]), span_of("a: int")),
                    ("b", CanonicalType::Named(vec!["utf8".to_string()]), span_of("b: utf8")),
                ],
                Some(CanonicalType::Named(vec!["bool".to_string()])),
                sub_source("query Find(a: int, b: utf8): bool"),
            ),
            operation(
                OperationKind::Udf,
                "Ping",
                Vec::new(),
                Some(CanonicalType::Named(vec!["utf8".to_string()])),
                sub_source("udf Ping(): utf8"),
            ),
            operation(OperationKind::Micro, "Reset", Vec::new(), None, sub_source("micro Reset()")),
        ],
    }
}

fn sub_source(declaration: &str) -> std::ops::Range<usize> {
    let start = SOURCE.find(declaration).unwrap_or_else(|| panic!("source contains {declaration}"));
    start..start + declaration.len()
}

fn fixture_json() -> String {
    parsed_artifact().to_json().expect("artifact serializes")
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

fn operation_id(index: usize) -> String {
    parsed_artifact().operations[index].operation_id.to_string()
}

// --- 计划形状 -----------------------------------------------------------

#[test]
fn plan_contains_generated_client_server_and_one_template() {
    let plan = generate_from_json(&fixture_json()).expect("generation succeeds");

    assert_eq!(plan.files().len(), 3);
    assert!(!contents_of(&plan, "hermes/operations.ts").is_empty());

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

    assert!(client.contains(&format!("  find: \"{}\",", operation_id(0))));
    assert!(client.contains(&format!("  ping: \"{}\",", operation_id(1))));
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
    assert!(server.contains(&format!("      operationId: \"{}\",", operation_id(0))));
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
fn method_and_operation_plans_share_a_root_without_overwriting_templates() {
    let operations = generate(&parsed_artifact()).expect("operation generation succeeds");
    let contract: ContractArtifact = serde_json::from_str(METHOD_FIXTURE).expect("contract fixture decodes");
    let contract = ContractArtifact::seal(ContractFormat::Hermes, contract.producer, contract.payload);
    let methods = generate_service(&contract, AotTarget::Desktop).expect("method generation succeeds");

    let operation_paths = operations.files().iter().map(|file| file.path()).collect::<BTreeSet<_>>();
    let method_paths = methods.files().iter().map(|file| file.path()).collect::<BTreeSet<_>>();
    let shared = operation_paths.intersection(&method_paths).copied().collect::<Vec<_>>();
    assert!(shared.is_empty(), "operation and method plans share {shared:?}");

    let root = temp_root("coexist");
    let operation_write = operations.write_to(&root).expect("operation write");
    let method_write = methods.write_to(&root).expect("method write");
    assert_eq!(operation_write.written().len(), operation_paths.len());
    assert_eq!(method_write.written().len(), method_paths.len());

    let operation_server = fs::read_to_string(root.join("hermes/server.ts")).expect("operation server readable");
    let method_server = fs::read_to_string(root.join("hermes/service/server.ts")).expect("method server readable");
    assert!(operation_server.contains("bindOperationHandlers"));
    assert!(method_server.contains("export function bindService"));

    let operation_template = root.join("hermes/handlers.ts");
    let method_template = root.join("hermes/service/handlers.ts");
    fs::write(&operation_template, "// operation handlers\n").expect("write operation handlers");
    fs::write(&method_template, "// method handlers\n").expect("write method handlers");

    let operation_again = operations.write_to(&root).expect("second operation write");
    assert!(operation_again.written().is_empty(), "operation write touched files: {:?}", operation_again.written());
    assert_eq!(operation_again.skipped(), &["hermes/handlers.ts".to_string()]);

    let method_again = methods.write_to(&root).expect("second method write");
    assert!(method_again.written().is_empty(), "method write touched files: {:?}", method_again.written());
    assert_eq!(method_again.skipped(), &["hermes/service/handlers.ts".to_string()]);

    assert_eq!(fs::read_to_string(&operation_template).expect("operation handlers readable"), "// operation handlers\n");
    assert_eq!(fs::read_to_string(&method_template).expect("method handlers readable"), "// method handlers\n");

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
    let empty = OperationProjectionArtifact {
        envelope: ContractEnvelope {
            stage: OPERATION_PROJECTION_STAGE.to_string(),
            contract_format_version: OPERATION_CONTRACT_FORMAT_VERSION.to_string(),
            language_version: LANGUAGE_VERSION.to_string(),
            canonicalization_version: CANONICALIZATION_VERSION.to_string(),
            schema_fingerprint: None,
            source_units: vec![SourceUnit { source_unit_id: 0, source: String::new() }],
        },
        operations: Vec::new(),
    }
    .to_json()
    .expect("empty artifact serializes");

    let plan = generate_from_json(&empty).expect("empty artifact is valid");
    assert!(contents_of(&plan, "hermes/operations.ts").contains("export const operationIds = {\n} as const;"));
    assert!(contents_of(&plan, "hermes/server.ts").contains("export {};"));
    assert!(contents_of(&plan, "hermes/handlers.ts").contains("export {};"));
}
