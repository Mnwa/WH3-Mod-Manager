use std::{hint::black_box, time::Instant};
use wh3_core::catalog::Catalog;

fn main() {
    println!("mods,build_ms,query_p50_ms,query_p95_ms");
    for count in [1_000, 10_000, 100_000] {
        let started = Instant::now();
        let catalog = Catalog::demo(count);
        let build = started.elapsed().as_secs_f64() * 1000.;
        let order: Vec<_> = (0..count).collect();
        let mut times = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            black_box(catalog.query(black_box("кислев 00"), &order));
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        println!("{count},{build:.3},{:.3},{:.3}", times[50], times[95]);
    }
}
