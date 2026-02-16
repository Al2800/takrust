use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustak_sim::{
    SweepAxis, SweepRunOptions, SweepRunner, TruthEngine, TruthEngineConfig, TruthState,
};

fn bench_sim_track_generation(criterion: &mut Criterion) {
    let axes = vec![
        SweepAxis::new("vx_mm_per_s", vec![80, 120, 160, 200]).expect("axis"),
        SweepAxis::new("vy_mm_per_s", vec![40, 90, 140]).expect("axis"),
    ];
    let runner = SweepRunner::new(axes, 0xfeed_beef).expect("runner");

    let mut group = criterion.benchmark_group("sim_track_generation");
    group.bench_function("truth_engine_sweep_case_generation", |bench| {
        bench.iter(|| {
            let report = runner.execute(SweepRunOptions::default(), |case| {
                let vx = *case
                    .parameters
                    .get("vx_mm_per_s")
                    .expect("velocity x axis should exist") as i32;
                let vy = *case
                    .parameters
                    .get("vy_mm_per_s")
                    .expect("velocity y axis should exist") as i32;

                let mut engine = TruthEngine::new(
                    case.case_id,
                    TruthState {
                        x_mm: 0,
                        y_mm: 0,
                        vx_mm_per_s: vx,
                        vy_mm_per_s: vy,
                    },
                    TruthEngineConfig::default(),
                )
                .expect("engine should initialize");

                let mut terminal = engine.state();
                for _ in 0..240 {
                    terminal = engine.advance().state;
                }
                terminal
            });

            black_box(report.executed_cases);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_sim_track_generation);
criterion_main!(benches);
