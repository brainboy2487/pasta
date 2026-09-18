use criterion::{criterion_group, criterion_main, Criterion};
use std::process::Command;

fn bench_pow(c: &mut Criterion) {
    let script = r#"
set s = 0
FOR i IN range(10000):
    set s = s + m_pow(1.0001, i)
END
PRINT s
"#;
    c.bench_function("pasta_m_pow_10k", |b| {
        b.iter(|| {
            let output = Command::new("target/debug/pasta")
                .args(["-e", script])
                .output()
                .expect("pasta run failed");
            assert!(output.status.success());
        })
    });
}

criterion_group!(benches, bench_pow);
criterion_main!(benches);
