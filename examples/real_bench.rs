use rocketsim_rs::sim::{Arena, CarConfig, Team};
use std::{
    thread::{available_parallelism, spawn},
    time::Instant,
};

fn main() {
    const TICKS: u32 = 200_000;

    // load in assets
    rocketsim_rs::init(None);

    let num_cpu = available_parallelism().unwrap().get();

    println!("Running on {num_cpu} threads");

    let start_time = Instant::now();
    let threads = (0..num_cpu)
        .map(|_| {
            spawn(|| {
                let mut arena = Arena::default_standard();

                let _ = arena.pin_mut().add_car(Team::Blue, CarConfig::octane());
                let _ = arena.pin_mut().add_car(Team::Blue, CarConfig::octane());
                let _ = arena.pin_mut().add_car(Team::Blue, CarConfig::octane());

                let _ = arena.pin_mut().add_car(Team::Orange, CarConfig::octane());
                let _ = arena.pin_mut().add_car(Team::Orange, CarConfig::octane());
                let _ = arena.pin_mut().add_car(Team::Orange, CarConfig::octane());

                arena.pin_mut().step(TICKS);
            })
        })
        .collect::<Vec<_>>();

    threads.into_iter().for_each(|thread| thread.join().unwrap());

    let elapsed = start_time.elapsed().as_secs_f32();
    let simulated_ticks = num_cpu as f32 * TICKS as f32;

    println!(
        "Simulated {:.2} hours in {:.3} seconds",
        simulated_ticks / 120. / 60. / 60.,
        elapsed
    );

    println!("FPS: {}", simulated_ticks / elapsed);
}
