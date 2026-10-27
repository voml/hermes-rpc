//! `ResolvedCanonicalType` 全分支 → TypeScript 形状符合性矩阵。
//!
//! 每个标量、`Optional` / `List` / `Reference` 的两层嵌套、已知与未知泛型、
//! user 类型的 nominal 引用与撞名，以及未冻结的 `json` / `value` / `any` 都在
//! 这里断言。无法映射的类型必须返回结构化错误，生成物里不允许出现 `unknown`。

use hermes_generator::{AotTarget, GenerationPlan, GeneratorError, generate_service};
use vos_types::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, ResolvedCanonicalType,
    ResolvedCanonicalTypeArgument,
};

const FIXTURE: &str = include_str!("fixtures/identity_service.contract.json");
const CLIENT: &str = "hermes/service/client.ts";
const SERVICE: &str = "hermes/service/service.ts";

fn artifact_with(mutate: impl FnOnce(&mut ContractPayload)) -> ContractArtifact {
    let parsed: ContractArtifact = serde_json::from_str(FIXTURE).expect("fixture decodes");
    let mut payload = parsed.payload;
    mutate(&mut payload);
    ContractArtifact::seal(ContractFormat::Hermes, ContractProducer::new("fixture", "0"), payload)
}

fn contents_of<'a>(plan: &'a GenerationPlan, path: &str) -> &'a str {
    plan.files().iter().find(|file| file.path() == path).unwrap_or_else(|| panic!("missing {path}")).contents()
}

/// 生成物里只允许 transport 桥接面固定用 `unknown`，canonical type 映射结果
/// 不得出现 `unknown`。错误消息里的 "unknown service method" 不是类型位置。
fn assert_no_unknown(plan: &GenerationPlan) {
    const TRANSPORT_LINES: [&str; 6] = [
        "readonly request: unknown;",
        "unary(call: HermesCall): Promise<unknown>;",
        "serverStream(call: HermesCall): AsyncIterable<unknown>;",
        "serverStream(call: HermesCall): ReadableStream<unknown>;",
        "readonly handler: (context: ServiceContext, request: unknown) => unknown;",
        "readonly handler: (ctx: HermesHandlerContext, params: unknown) => unknown;",
    ];
    const TYPE_POSITIONS: [&str; 10] = [
        "unknown,",
        "unknown)",
        "unknown;",
        "unknown>",
        "unknown[",
        "unknown |",
        "| unknown",
        "<unknown",
        ": unknown",
        "(unknown",
    ];

    for file in plan.files() {
        for line in file.contents().lines() {
            if !TYPE_POSITIONS.iter().any(|pattern| line.contains(pattern)) {
                continue;
            }
            let trimmed = line.trim();
            assert!(TRANSPORT_LINES.contains(&trimmed), "unexpected `unknown` type in {}: `{trimmed}`", file.path());
        }
    }
}

/// 只替换 `create` 的请求类型，其余合同保持合法。
fn request_ts(ty: ResolvedCanonicalType) -> String {
    let artifact = artifact_with(|payload| payload.services[0].methods[0].request_type = ty);
    let plan = generate_service(&artifact, AotTarget::Desktop).expect("mappable type must generate");
    assert_no_unknown(&plan);
    client_request_type(&plan)
}

/// 只替换 `create` 的请求类型，断言生成被结构化错误拒绝并返回 `(code, message)`。
fn request_error(ty: ResolvedCanonicalType) -> (String, String) {
    let artifact = artifact_with(|payload| payload.services[0].methods[0].request_type = ty);
    match generate_service(&artifact, AotTarget::Desktop).expect_err("unmappable type must be rejected") {
        GeneratorError::Artifact(error) => (error.code, error.message),
        other => panic!("expected a structured mapping error, got {other}"),
    }
}

fn client_request_type(plan: &GenerationPlan) -> String {
    let client = contents_of(plan, CLIENT);
    let signature =
        client.lines().find(|line| line.trim_start().starts_with("create(request: ")).expect("client declares create");
    let rest = signature.trim_start().strip_prefix("create(request: ").expect("request parameter prefix");
    let end = rest.find(", options?: CallOptions)").expect("request parameter suffix");
    rest[..end].to_string()
}

fn scalar(scalar: CanonicalScalar) -> ResolvedCanonicalType {
    ResolvedCanonicalType::Scalar(scalar)
}

fn user(path: &[&str], type_id: u64) -> ResolvedCanonicalType {
    ResolvedCanonicalType::User { path: path.iter().map(|segment| segment.to_string()).collect(), type_id }
}

fn identity() -> ResolvedCanonicalType {
    user(&["demo", "auth", "Identity"], 2)
}

fn optional(inner: ResolvedCanonicalType) -> ResolvedCanonicalType {
    ResolvedCanonicalType::Optional(Box::new(inner))
}

fn list(inner: ResolvedCanonicalType) -> ResolvedCanonicalType {
    ResolvedCanonicalType::List(Box::new(inner))
}

fn reference(inner: ResolvedCanonicalType) -> ResolvedCanonicalType {
    ResolvedCanonicalType::Reference(Box::new(inner))
}

fn generic(name: &str, arguments: Vec<ResolvedCanonicalType>) -> ResolvedCanonicalType {
    ResolvedCanonicalType::Generic {
        path: vec![name.to_string()],
        arguments: arguments.into_iter().map(ResolvedCanonicalTypeArgument::Type).collect(),
    }
}

fn expected_scalar_ts(scalar: CanonicalScalar) -> &'static str {
    match scalar {
        CanonicalScalar::I8
        | CanonicalScalar::I16
        | CanonicalScalar::I32
        | CanonicalScalar::U8
        | CanonicalScalar::U16
        | CanonicalScalar::U32
        | CanonicalScalar::F32
        | CanonicalScalar::F64 => "number",
        CanonicalScalar::Bool => "boolean",
        CanonicalScalar::Utf8
        | CanonicalScalar::Utf16
        | CanonicalScalar::I64
        | CanonicalScalar::U64
        | CanonicalScalar::Decimal
        | CanonicalScalar::Uuid
        | CanonicalScalar::Date
        | CanonicalScalar::Time
        | CanonicalScalar::DateTime => "string",
        CanonicalScalar::Bytes => "Uint8Array",
    }
}

// --- 标量 ---------------------------------------------------------------

#[test]
fn every_scalar_branch_maps_to_its_typescript_primitive() {
    assert_eq!(CanonicalScalar::ALL.len(), 19, "the matrix must cover every scalar branch");

    for value in CanonicalScalar::ALL {
        let ts = request_ts(scalar(value));
        assert_eq!(ts, expected_scalar_ts(value), "scalar `{}` mapped to `{ts}`", value.as_str());
    }
}

// --- 包装 ---------------------------------------------------------------

#[test]
fn optional_list_and_reference_compose_two_levels_deep() {
    let cases = [
        (optional(scalar(CanonicalScalar::I64)), "string | null"),
        (list(scalar(CanonicalScalar::I32)), "number[]"),
        (list(optional(scalar(CanonicalScalar::I32))), "(number | null)[]"),
        (optional(list(scalar(CanonicalScalar::I32))), "number[] | null"),
        (list(list(scalar(CanonicalScalar::Uuid))), "string[][]"),
        (optional(optional(scalar(CanonicalScalar::Uuid))), "(string | null) | null"),
        (reference(scalar(CanonicalScalar::Utf8)), "string"),
        (reference(list(optional(scalar(CanonicalScalar::I32)))), "(number | null)[]"),
        (reference(reference(identity())), "DemoAuthIdentity"),
        (optional(list(optional(identity()))), "((DemoAuthIdentity | null)[]) | null"),
    ];

    for (ty, expected) in cases {
        assert_eq!(request_ts(ty), expected, "unexpected shape for `{expected}`");
    }
}

// --- 泛型 ---------------------------------------------------------------

#[test]
fn known_generics_map_to_typescript_containers() {
    let cases = [
        (generic("List", vec![scalar(CanonicalScalar::I32)]), "number[]"),
        (generic("array", vec![scalar(CanonicalScalar::Utf8)]), "string[]"),
        (generic("Vec", vec![scalar(CanonicalScalar::Uuid)]), "string[]"),
        (generic("Optional", vec![scalar(CanonicalScalar::I64)]), "string | null"),
        (generic("option", vec![scalar(CanonicalScalar::I32)]), "number | null"),
        (generic("Map", vec![scalar(CanonicalScalar::Utf8), scalar(CanonicalScalar::I32)]), "Record<string, number>"),
        (
            generic("record", vec![scalar(CanonicalScalar::Utf8), optional(scalar(CanonicalScalar::I32))]),
            "Record<string, number | null>",
        ),
        (generic("dict", vec![scalar(CanonicalScalar::Utf8), list(scalar(CanonicalScalar::Uuid))]), "Record<string, string[]>"),
        (generic("List", vec![generic("Optional", vec![scalar(CanonicalScalar::I32)])]), "(number | null)[]"),
        (generic("optional", vec![generic("list", vec![identity()])]), "DemoAuthIdentity[] | null"),
    ];

    for (ty, expected) in cases {
        assert_eq!(request_ts(ty), expected, "unexpected shape for `{expected}`");
    }
}

#[test]
fn unknown_generic_is_rejected_instead_of_degrading_to_unknown() {
    let (code, message) = request_error(generic("Set", vec![scalar(CanonicalScalar::I32)]));
    assert_eq!(code, "GEN002");
    assert!(message.contains("method `IdentityService.create` request: generic type `Set`"), "{message}");
    assert!(!message.contains("unknown"), "the error must not describe an `unknown` fallback: {message}");

    let (code, _) = request_error(generic("list", vec![scalar(CanonicalScalar::I32), scalar(CanonicalScalar::I32)]));
    assert_eq!(code, "GEN002", "known container names with the wrong arity are still unmappable");

    let hollow = ResolvedCanonicalType::Generic {
        path: vec!["vector".to_string()],
        arguments: vec![ResolvedCanonicalTypeArgument::Literal("3".to_string())],
    };
    let (code, message) = request_error(hollow);
    assert_eq!(code, "GEN003");
    assert!(message.contains("literal generic argument `3`"), "{message}");
}

// --- user 类型 ----------------------------------------------------------

#[test]
fn user_types_bind_stable_nominal_aliases() {
    let artifact = artifact_with(|payload| {
        let service = &mut payload.services[0];
        service.methods[0].response_type = identity();
        service.methods[1].request_type = identity();
    });
    let plan = generate_service(&artifact, AotTarget::Desktop).expect("nominal references are stable");
    assert_no_unknown(&plan);

    let service_ts = contents_of(&plan, SERVICE);
    assert_eq!(service_ts.matches("export type DemoAuthIdentity = { readonly __vosTypeId: 2 };").count(), 1);
    assert!(
        contents_of(&plan, CLIENT)
            .contains("create(request: DemoAuthCreateRequest, options?: CallOptions): Promise<DemoAuthIdentity> {")
    );
    assert!(
        contents_of(&plan, CLIENT)
            .contains("lookup(request: DemoAuthIdentity, options?: CallOptions): AsyncIterable<DemoAuthIdentity | null> {")
    );
    assert!(
        plan.diagnostics().iter().any(|diagnostic| diagnostic.message().contains("user type `demo.auth.Identity`")
            && diagnostic.message().contains("nominal reference `DemoAuthIdentity`")),
        "expected a nominal user type diagnostic, got {:?}",
        plan.diagnostics()
    );

    let again = generate_service(&artifact, AotTarget::Desktop).expect("generation is deterministic");
    let snapshot = |plan: &GenerationPlan| {
        plan.files().iter().map(|file| (file.path().to_string(), file.contents().to_string())).collect::<Vec<_>>()
    };
    assert_eq!(snapshot(&plan), snapshot(&again));
}

#[test]
fn same_type_name_in_different_paths_keeps_distinct_aliases() {
    let artifact = artifact_with(|payload| {
        let service = &mut payload.services[0];
        service.methods[0].request_type = user(&["demo", "a", "Item"], 11);
        service.methods[1].request_type = user(&["demo", "b", "Item"], 12);
    });
    let plan = generate_service(&artifact, AotTarget::Desktop).expect("different paths must not collide");
    assert_no_unknown(&plan);

    let service_ts = contents_of(&plan, SERVICE);
    assert!(service_ts.contains("export type DemoAItem = { readonly __vosTypeId: 11 };"));
    assert!(service_ts.contains("export type DemoBItem = { readonly __vosTypeId: 12 };"));
    assert!(contents_of(&plan, CLIENT).contains("create(request: DemoAItem, options?: CallOptions)"));
    assert!(contents_of(&plan, CLIENT).contains("lookup(request: DemoBItem, options?: CallOptions)"));
}

#[test]
fn normalized_user_type_collision_is_rejected() {
    let artifact = artifact_with(|payload| {
        let service = &mut payload.services[0];
        service.methods[0].request_type = user(&["A", "B"], 21);
        service.methods[1].request_type = user(&["a", "b"], 22);
    });

    match generate_service(&artifact, AotTarget::Desktop).expect_err("`A.B` and `a.b` normalize to the same alias") {
        GeneratorError::NamingCollision { identifier, operations } => {
            assert_eq!(identifier, "AB");
            assert!(operations.contains(&"A.B".to_string()), "{operations:?}");
            assert!(operations.contains(&"a.b".to_string()), "{operations:?}");
        }
        other => panic!("expected a naming collision, got {other}"),
    }
}

// --- 未冻结类型 ---------------------------------------------------------

#[test]
fn unfrozen_json_value_and_any_are_rejected() {
    for name in ["json", "value", "any"] {
        let (code, message) = request_error(user(&["demo", name], 31));
        assert_eq!(code, "GEN001", "user `{name}` must carry an explicit rejection code");
        assert!(message.contains(&format!("unfrozen type `demo.{name}`")), "{message}");
        assert!(!message.contains("unknown"), "no `unknown` fallback may be described: {message}");

        let (code, message) =
            request_error(ResolvedCanonicalType::Generic { path: vec![name.to_string()], arguments: Vec::new() });
        assert_eq!(code, "GEN001", "generic `{name}` must carry an explicit rejection code");
        assert!(message.contains(&format!("unfrozen type `{name}`")), "{message}");
        assert!(!message.contains("unknown"), "no `unknown` fallback may be described: {message}");
    }
}

#[test]
fn generated_service_never_contains_unknown() {
    let plan = generate_service(&artifact_with(|_| {}), AotTarget::Desktop).expect("fixture generates");
    assert_no_unknown(&plan);
}
