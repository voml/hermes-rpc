//! method 级 AOT 生成测试。
//!
//! fixture 直接来自上游合同形状，测试先用上游 `seal` 重新封装成 Hermes
//! artifact，再验证生成文件集合、宿主差异、route 拒绝与诊断。method 级生成物
//! 全部落在 `hermes/service/` 子目录，不允许占用 operation 级的任何路径。

use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use hermes_generator::{AotTarget, FileOwnership, GenerationPlan, GeneratorError, generate_service};
use vos_types::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, MethodId, MethodIdentityKey,
    OperationId, ResolvedCanonicalType, ResolvedCanonicalTypeArgument, service_contract_fingerprint,
};

const FIXTURE: &str = include_str!("fixtures/identity_service.contract.json");
const SERVICE_ID: &str = "svc_9840335d138fd4c32651c498ecd3f0c9aa05cf5433828d9df268aad47c9cfbc1";
const LOOKUP_METHOD: &str = "mtd_8209e030c55458c829b98b16cd5cb582db22fca4ec29fcc991241dfc36ce3ff8";

/// operation 级生成器占用的路径，method 级不得使用其中任何一个。
const OPERATION_LEVEL_PATHS: [&str; 3] = ["hermes/operations.ts", "hermes/server.ts", "hermes/handlers.ts"];

fn fixture_artifact() -> ContractArtifact {
    let parsed: ContractArtifact = serde_json::from_str(FIXTURE).expect("fixture decodes");
    ContractArtifact::seal(ContractFormat::Hermes, parsed.producer, parsed.payload)
}

fn artifact_with(mutate: impl FnOnce(&mut ContractPayload)) -> ContractArtifact {
    let artifact = fixture_artifact();
    let mut payload = artifact.payload;
    mutate(&mut payload);
    ContractArtifact::seal(ContractFormat::Hermes, ContractProducer::new("fixture", "0"), payload)
}

fn contents_of<'a>(plan: &'a GenerationPlan, path: &str) -> &'a str {
    plan.files().iter().find(|file| file.path() == path).unwrap_or_else(|| panic!("missing {path}")).contents()
}

fn temp_root(tag: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after epoch").as_nanos();
    let root = std::env::temp_dir().join(format!("hermes-generator-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

// --- 文件集合 -----------------------------------------------------------

#[test]
fn plan_covers_service_client_routes_server_metadata_and_template() {
    let plan = generate_service(&fixture_artifact(), AotTarget::Desktop).expect("generation succeeds");
    let ownership = plan.files().iter().map(|file| (file.path(), file.ownership())).collect::<Vec<_>>();

    assert_eq!(
        ownership,
        vec![
            ("hermes/service/service.ts", FileOwnership::Generated),
            ("hermes/service/client.ts", FileOwnership::Generated),
            ("hermes/service/routes.ts", FileOwnership::Generated),
            ("hermes/service/server.ts", FileOwnership::Generated),
            ("hermes/service/metadata.ts", FileOwnership::Generated),
            ("hermes/service/handlers.ts", FileOwnership::Template),
        ]
    );
    assert!(
        plan.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message().contains("request: user type `demo.auth.CreateRequest`")),
        "expected a nominal user type diagnostic, got {:?}",
        plan.diagnostics()
    );
}

#[test]
fn method_level_outputs_stay_inside_their_own_directory() {
    let plan = generate_service(&fixture_artifact(), AotTarget::Desktop).expect("generation succeeds");

    for file in plan.files() {
        assert!(file.path().starts_with("hermes/service/"), "{} is outside the method-level directory", file.path());
        assert!(!OPERATION_LEVEL_PATHS.contains(&file.path()), "{} collides with an operation-level output", file.path());
    }
}

#[test]
fn generation_is_deterministic_for_both_targets() {
    let artifact = fixture_artifact();
    for target in [AotTarget::Desktop, AotTarget::Web] {
        let first = generate_service(&artifact, target).expect("first generation");
        let second = generate_service(&artifact, target).expect("second generation");
        let first_files = first.files().iter().map(|file| (file.path(), file.contents())).collect::<Vec<_>>();
        let second_files = second.files().iter().map(|file| (file.path(), file.contents())).collect::<Vec<_>>();
        assert_eq!(first_files, second_files);
    }
}

// --- service / metadata -------------------------------------------------

#[test]
fn service_module_exports_identities_fingerprints_and_user_types() {
    let artifact = fixture_artifact();
    let plan = generate_service(&artifact, AotTarget::Desktop).expect("generation succeeds");
    let service = contents_of(&plan, "hermes/service/service.ts");

    assert!(service.contains("export const contractVersion = \"vos/hermes.contract/v0\";"));
    assert!(service.contains(&format!("  identityService: \"{SERVICE_ID}\",")));
    assert!(service.contains(&format!("    lookup: \"{LOOKUP_METHOD}\",")));
    assert!(service.contains("export type DemoAuthCreateRequest = { readonly __vosTypeId: 1 };"));
    assert!(service.contains("export type DemoAuthIdentity = { readonly __vosTypeId: 2 };"));
    assert!(service.contains("export type ServiceKey = keyof typeof serviceIds;"));
    assert!(service.contains("export type MethodId ="));

    let service_contract = &artifact.payload.services[0];
    let service_fingerprint = service_contract_fingerprint(std::slice::from_ref(service_contract));
    assert!(service.contains(&format!("  identityService: \"{service_fingerprint}\",")));

    let mut scoped = service_contract.clone();
    scoped.methods = vec![scoped.methods[0].clone()];
    let method_fingerprint = service_contract_fingerprint(std::slice::from_ref(&scoped));
    assert!(service.contains(&format!("    create: \"{method_fingerprint}\",")));
}

#[test]
fn metadata_records_revision_provenance_and_fingerprints() {
    let artifact = fixture_artifact();
    let plan = generate_service(&artifact, AotTarget::Desktop).expect("generation succeeds");
    let metadata = contents_of(&plan, "hermes/service/metadata.ts");

    assert!(metadata.contains(&format!("export const contractFingerprint = \"{}\";", artifact.fingerprint)));
    assert!(metadata.contains("export const generatorRevision = \"hermes-generator@"));
    assert!(metadata.contains("export const aotTarget = \"desktop\" as const;"));
    assert!(metadata.contains("  producerName: \"fixture\","));
    assert!(metadata.contains("    canonicalPath: [\"demo\", \"auth\", \"IdentityService\"],"));
    assert!(metadata.contains("        stream: \"server-stream\","));
}

// --- client / server 宿主差异 --------------------------------------------

#[test]
fn client_emits_typed_unary_and_server_stream_methods() {
    let plan = generate_service(&fixture_artifact(), AotTarget::Desktop).expect("generation succeeds");
    let client = contents_of(&plan, "hermes/service/client.ts");

    assert!(client.contains("export class IdentityServiceClient"));
    assert!(client.contains("  create(request: DemoAuthCreateRequest, options?: CallOptions): Promise<DemoAuthIdentity> {"));
    assert!(client.contains("  lookup(request: string, options?: CallOptions): AsyncIterable<DemoAuthIdentity | null> {"));
    assert!(client.contains("    return this.dispatch.unary({"));
    assert!(client.contains("    return this.dispatch.serverStream({"));
    assert!(client.contains("      methodId: methodIds.identityService.lookup,"));
    assert!(client.contains("  readonly deadline?: number;"));
    assert!(client.contains("  readonly signal?: AbortSignal;"));
    assert!(client.contains("  readonly metadata?: Readonly<Record<string, string>>;"));
}

#[test]
fn web_target_uses_readable_stream_while_desktop_uses_async_iterable() {
    let artifact = fixture_artifact();
    let desktop = generate_service(&artifact, AotTarget::Desktop).expect("desktop generation");
    let web = generate_service(&artifact, AotTarget::Web).expect("web generation");

    assert!(contents_of(&desktop, "hermes/service/client.ts").contains("AsyncIterable<DemoAuthIdentity | null>"));
    assert!(
        contents_of(&desktop, "hermes/service/server.ts")
            .contains("lookup(context: ServiceContext, request: string): AsyncIterable<DemoAuthIdentity | null>;")
    );

    assert!(contents_of(&web, "hermes/service/client.ts").contains("ReadableStream<DemoAuthIdentity | null>"));
    assert!(
        contents_of(&web, "hermes/service/server.ts")
            .contains("lookup(context: ServiceContext, request: string): ReadableStream<DemoAuthIdentity | null>;")
    );

    for file in desktop.files() {
        assert!(!file.contents().contains("ReadableStream"), "desktop {} leaks the web stream type", file.path());
    }
    for file in web.files() {
        assert!(!file.contents().contains("AsyncIterable"), "web {} leaks the desktop stream type", file.path());
    }
    assert!(contents_of(&web, "hermes/service/metadata.ts").contains("export const aotTarget = \"web\" as const;"));
}

#[test]
fn generated_imports_stay_inside_the_generated_modules() {
    let plan = generate_service(&fixture_artifact(), AotTarget::Web).expect("generation succeeds");

    for file in plan.files() {
        for line in file.contents().lines().filter(|line| line.starts_with("import ")) {
            assert!(line.contains("from \"./"), "{} imports outside the generated directory: {line}", file.path());
        }
        for forbidden in ["@yydb/hermes", "hermes-wasm", "vos-language", "node:", "wasm_bindgen"] {
            assert!(!file.contents().contains(forbidden), "{} references `{forbidden}`", file.path());
        }
    }
}

// --- routes / bindings --------------------------------------------------

#[test]
fn routes_table_resolves_known_methods_and_rejects_unknown_ones() {
    let plan = generate_service(&fixture_artifact(), AotTarget::Desktop).expect("generation succeeds");
    let routes = contents_of(&plan, "hermes/service/routes.ts");

    assert!(routes.contains(&format!("  \"{SERVICE_ID}/create\": {{")));
    assert!(routes.contains("    stream: \"server-stream\","));
    assert!(routes.contains("export function resolveRoute(serviceName: string, methodName: string): ServiceRoute | undefined"));
    assert!(routes.contains("export function requireRoute(serviceName: string, methodName: string): ServiceRoute"));
    assert!(routes.contains("throw new Error(`unknown service method: ${serviceName}.${methodName}`);"));
    assert!(routes.contains("export function methodIdOf(serviceId: string, methodName: string): string | undefined"));

    let server = contents_of(&plan, "hermes/service/server.ts");
    assert!(server.contains("  identityService: IdentityServiceHandlers;"));
    assert!(server.contains("route: requireRoute(\"IdentityService\", \"lookup\"),"));
    assert!(server.contains("export function assertOperationBindings("));
}

#[test]
fn host_route_keys_are_unique_mount_keys_covering_every_method() {
    let plan = generate_service(&fixture_artifact(), AotTarget::Desktop).expect("generation succeeds");
    let routes = contents_of(&plan, "hermes/service/routes.ts");

    let keys = routes
        .lines()
        .filter_map(|line| line.strip_prefix("  \""))
        .filter_map(|line| line.strip_suffix("\": {"))
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 2, "the fixture declares two methods, got {keys:?}");
    assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), keys.len(), "host route keys must not collide");
    assert!(keys.contains(&format!("{SERVICE_ID}/create").as_str()));
    assert!(keys.contains(&format!("{SERVICE_ID}/lookup").as_str()));
}

#[test]
fn handler_template_is_one_time_and_marks_unimplemented_methods() {
    let plan = generate_service(&fixture_artifact(), AotTarget::Desktop).expect("generation succeeds");
    let template = contents_of(&plan, "hermes/service/handlers.ts");

    assert!(template.contains("This file is never overwritten."));
    assert!(template.contains("throw new Error(\"method `IdentityService.lookup` is not implemented\");"));

    let root = temp_root("methods");
    let first = plan.write_to(&root).expect("first write");
    assert_eq!(first.written().len(), 6);

    let handler_path = root.join("hermes/service/handlers.ts");
    fs::write(&handler_path, "// user handlers\n").expect("write user handlers");

    let second = plan.write_to(&root).expect("second write");
    assert!(second.written().is_empty(), "nothing changed, got {:?}", second.written());
    assert_eq!(second.skipped(), &["hermes/service/handlers.ts".to_string()]);
    assert_eq!(second.unchanged().len(), 5);
    assert_eq!(fs::read_to_string(&handler_path).expect("handlers readable"), "// user handlers\n");

    fs::remove_dir_all(&root).expect("cleanup temp root");
}

// --- 合同拒绝与诊断 -----------------------------------------------------

#[test]
fn rejects_non_hermes_artifacts() {
    let parsed: ContractArtifact = serde_json::from_str(FIXTURE).expect("fixture decodes");
    let iris = ContractArtifact::seal(ContractFormat::Iris, parsed.producer, parsed.payload);

    match generate_service(&iris, AotTarget::Desktop).expect_err("Iris artifact must be rejected") {
        GeneratorError::UnsupportedContractFormat { found } => assert_eq!(found, "vos/iris.contract/v0"),
        other => panic!("expected UnsupportedContractFormat, got {other}"),
    }
}

#[test]
fn rejects_methods_that_normalize_to_the_same_route() {
    let artifact = artifact_with(|payload| {
        let service = &mut payload.services[0];
        let service_id = service.service_id.clone();
        for (method, name) in service.methods.iter_mut().zip(["fetch-notes", "fetch.notes"]) {
            method.name = name.to_string();
            method.method_id =
                MethodId::derive(&MethodIdentityKey::new(service_id.clone(), name).expect("method identity key"));
        }
    });

    match generate_service(&artifact, AotTarget::Desktop).expect_err("duplicate route must be rejected") {
        GeneratorError::DuplicateRoute { route } => assert_eq!(route, "IdentityService.fetchNotes"),
        other => panic!("expected DuplicateRoute, got {other}"),
    }
}

#[test]
fn rejects_methods_bound_to_operations_outside_the_payload() {
    let artifact = artifact_with(|payload| {
        let synthetic = format!("op_{}", "0".repeat(64));
        payload.services[0].methods[1].operation_id = OperationId::parse(&synthetic).expect("synthetic operation id");
    });

    match generate_service(&artifact, AotTarget::Desktop).expect_err("missing operation must be rejected") {
        // 上游 `validate` 已把悬空 operation binding 归类为 artifact 错误。
        GeneratorError::Artifact(err) => assert!(
            err.message.contains("service method lookup binds unknown operation"),
            "unexpected artifact error: {}",
            err.message
        ),
        // 生成器自身的防御检查保留同义拒绝。
        GeneratorError::MissingOperationBinding { service, method, .. } => {
            assert_eq!(service, "IdentityService");
            assert_eq!(method, "lookup");
        }
        other => panic!("expected the dangling operation binding to be rejected, got {other}"),
    }
}

#[test]
fn unmapped_generic_types_report_diagnostics_and_keep_unknown() {
    let artifact = artifact_with(|payload| {
        payload.services[0].methods[0].request_type = ResolvedCanonicalType::Generic {
            path: vec!["Set".to_string()],
            arguments: vec![ResolvedCanonicalTypeArgument::Type(ResolvedCanonicalType::Scalar(CanonicalScalar::I32))],
        };
    });

    let plan = generate_service(&artifact, AotTarget::Desktop).expect("generation still succeeds");
    assert!(
        plan.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message().contains("request: generic type `Set` cannot be mapped to TypeScript")),
        "expected a generic mapping diagnostic, got {:?}",
        plan.diagnostics()
    );
    assert!(contents_of(&plan, "hermes/service/client.ts").contains("create(request: unknown, options?: CallOptions)"));
}
