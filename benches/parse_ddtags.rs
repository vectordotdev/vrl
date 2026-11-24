//! Compares the native `parse_ddtags` function with the same parsing written in plain VRL.
use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use vrl::compiler::runtime::Runtime;
use vrl::compiler::{Program, TargetValue, TimeZone, compile};
use vrl::value::{ObjectMap, Secrets, Value};

// ~50 realistic Datadog tags with long values and duplicate keys.
const DDTAGS: &str = "\
env:production,\
service:payment-gateway-service,\
version:4.12.7-rc3,\
host:ip-10-42-137-29.us-east-1.compute.internal,\
instance-type:m5.2xlarge,\
availability-zone:us-east-1c,\
region:us-east-1,\
cluster:eks-prod-main-useast1-2024,\
namespace:payments,\
pod_name:payment-gateway-service-7f8b9c6d4f-x2k9m,\
container_name:payment-gateway,\
image_tag:registry.internal.example.com/payments/gateway:4.12.7-rc3-sha-a1b2c3d4,\
team:platform-payments,\
cost_center:cc-payments-12345,\
owner:payments-team@example.com,\
pagerduty:payments-p1,\
slo:payments-availability-99.99,\
tier:tier-0-critical,\
compliance:pci-dss-v4,\
compliance:soc2-type2,\
compliance:gdpr,\
datacenter:us-east-1-primary,\
network:vpc-0a1b2c3d4e5f67890,\
subnet:subnet-private-us-east-1c-payments,\
security_group:sg-payment-gateway-prod,\
load_balancer:arn:aws:elasticloadbalancing:us-east-1:123456789012:loadbalancer/app/payment-gw-prod/50dc6c495c0c9188,\
target_group:arn:aws:elasticloadbalancing:us-east-1:123456789012:targetgroup/payment-gw-tg/73e2d6bc24d8a067,\
dns:payment-gateway.internal.prod.example.com,\
port:8443,\
protocol:https,\
framework:spring-boot-3.2.1,\
runtime:openjdk-21.0.2+13,\
orchestrator:kubernetes-1.29,\
deploy_pipeline:argo-cd,\
deploy_sha:a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0,\
deploy_timestamp:2024-11-15T14:32:07Z,\
canary:false,\
feature_flag:new-checkout-flow-v2,\
feature_flag:payment-retry-logic-v3,\
feature_flag:fraud-detection-ml-model-2024q4,\
circuit_breaker:downstream-bank-api,\
rate_limit_tier:premium,\
db_pool:payments-primary-rds-cluster.cluster-abc123def456.us-east-1.rds.amazonaws.com,\
cache_cluster:payments-redis-prod-001.abc123.0001.use1.cache.amazonaws.com,\
message_queue:arn:aws:sqs:us-east-1:123456789012:payment-events-prod,\
trace_sample_rate:0.15,\
log_level:info,\
custom_metric_prefix:payments.gateway,\
git_repository:github.com/example-org/payment-gateway-service,\
oncall_schedule:payments-primary-rotation-2024";

const NATIVE: &str = ".parsed = parse_ddtags!(string!(.ddtags))";

const PURE_VRL: &str = r#"
    result = {}
    for_each(split(string!(.ddtags), ",")) -> |_i, tag| {
        parts = split(tag, ":", limit: 2)
        key = strip_whitespace(string!(parts[0]))
        val = if length(parts) > 1 { strip_whitespace(string!(parts[1])) } else { true }
        existing = get!(result, [key])
        if existing == null {
            result = set!(result, [key], [val])
        } else {
            result = set!(result, [key], push(array!(existing), val))
        }
    }
    .parsed = result
"#;

fn compile_vrl(source: &str) -> Program {
    compile(source, &vrl::stdlib::all())
        .unwrap_or_else(|err| panic!("failed to compile VRL: {err:?}"))
        .program
}

fn target() -> TargetValue {
    let mut event = ObjectMap::new();
    event.insert("ddtags".into(), Value::from(DDTAGS));
    TargetValue {
        value: Value::Object(event),
        metadata: Value::Object(ObjectMap::new()),
        secrets: Secrets::default(),
    }
}

fn resolve(runtime: &mut Runtime, program: &Program) -> TargetValue {
    let mut target = target();
    runtime
        .resolve(&mut target, program, &TimeZone::default())
        .expect("program runs");
    runtime.clear();
    target
}

fn bench_parse_ddtags(c: &mut Criterion) {
    let mut runtime = Runtime::default();
    let native = compile_vrl(NATIVE);
    let pure_vrl = compile_vrl(PURE_VRL);

    // Both programs must give the same result, or the comparison means nothing.
    assert_eq!(
        resolve(&mut runtime, &native).value,
        resolve(&mut runtime, &pure_vrl).value
    );

    let mut group = c.benchmark_group("parse_ddtags");
    for (name, program) in [("native", &native), ("pure_vrl", &pure_vrl)] {
        group.bench_function(name, |b| {
            b.iter_batched(
                target,
                |mut t| {
                    black_box(runtime.resolve(&mut t, program, &TimeZone::default())).unwrap();
                    runtime.clear();
                    t
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse_ddtags);
criterion_main!(benches);
